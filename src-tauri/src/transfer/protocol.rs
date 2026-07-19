use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::manifest::{self, WireTransferManifest};
use super::{chunk, integrity, resume};
use crate::error::{AppError, AppResult};
use crate::transport::session::NoiseSession;

const TAG_MANIFEST: u8 = 1;
const TAG_MISSING_CHUNKS: u8 = 2;
const TAG_CHUNK: u8 = 3;
const TAG_DONE: u8 = 4;
const TAG_ACCEPT: u8 = 5;
const TAG_REJECT: u8 = 6;
const TAG_TEXT: u8 = 7;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MissingChunksPayload {
    file_index: u32,
    indices: Vec<u32>,
}

enum WireMessage {
    Manifest(WireTransferManifest),


    Accept,
    Reject,
    MissingChunks(MissingChunksPayload),
    Chunk {
        file_index: u32,
        chunk_index: u32,
        hash: [u8; 32],
        data: Vec<u8>,
    },
    Done,


    Text {
        text: String,
    },
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendOutcome {
    Completed,

    Rejected,

    Cancelled,
}


#[derive(Debug, Clone, Copy, Default)]
pub struct SendProgress {
    pub bytes_sent: u64,
    pub local_micros: u64,
    pub encrypt_micros: u64,
    pub socket_micros: u64,
}


#[derive(Debug, Clone, Copy, Default)]
pub struct RecvProgress {
    pub bytes_written: u64,
    pub recv_wait_micros: u64,
    pub disk_write_micros: u64,
}


pub enum ReceiveOutcome {
    Completed {
        files: Vec<PathBuf>,


        folder_archive_name: Option<String>,


        total_bytes: u64,
    },
    Rejected,


    TextMessage(String),
}

impl WireMessage {
    fn encode(&self) -> AppResult<Vec<u8>> {
        match self {
            WireMessage::Manifest(m) => {
                let mut out = vec![TAG_MANIFEST];
                out.extend(serde_json::to_vec(m).map_err(|e| {
                    AppError::new(format!("failed to encode manifest message: {e}"))
                })?);
                Ok(out)
            }
            WireMessage::MissingChunks(p) => {
                let mut out = vec![TAG_MISSING_CHUNKS];
                out.extend(serde_json::to_vec(p).map_err(|e| {
                    AppError::new(format!("failed to encode missing-chunks message: {e}"))
                })?);
                Ok(out)
            }
            WireMessage::Chunk {
                file_index,
                chunk_index,
                hash,
                data,
            } => {
                let mut out = Vec::with_capacity(1 + 4 + 4 + 32 + data.len());
                out.push(TAG_CHUNK);
                out.extend_from_slice(&file_index.to_be_bytes());
                out.extend_from_slice(&chunk_index.to_be_bytes());
                out.extend_from_slice(hash);
                out.extend_from_slice(data);
                Ok(out)
            }
            WireMessage::Done => Ok(vec![TAG_DONE]),
            WireMessage::Accept => Ok(vec![TAG_ACCEPT]),
            WireMessage::Reject => Ok(vec![TAG_REJECT]),
            WireMessage::Text { text } => {
                let mut out = vec![TAG_TEXT];
                out.extend_from_slice(text.as_bytes());
                Ok(out)
            }
        }
    }

    fn decode(bytes: &[u8]) -> AppResult<Self> {
        let (&tag, rest) = bytes
            .split_first()
            .ok_or_else(|| AppError::new("empty wire message"))?;

        match tag {
            TAG_MANIFEST => {
                let m = serde_json::from_slice(rest)
                    .map_err(|e| AppError::new(format!("failed to decode manifest: {e}")))?;
                Ok(WireMessage::Manifest(m))
            }
            TAG_MISSING_CHUNKS => {
                let p = serde_json::from_slice(rest).map_err(|e| {
                    AppError::new(format!("failed to decode missing-chunks message: {e}"))
                })?;
                Ok(WireMessage::MissingChunks(p))
            }
            TAG_CHUNK => {
                if rest.len() < 40 {
                    return Err(AppError::new("truncated chunk message"));
                }
                let file_index = u32::from_be_bytes(rest[0..4].try_into().unwrap());
                let chunk_index = u32::from_be_bytes(rest[4..8].try_into().unwrap());
                let hash: [u8; 32] = rest[8..40].try_into().unwrap();
                let data = rest[40..].to_vec();
                Ok(WireMessage::Chunk {
                    file_index,
                    chunk_index,
                    hash,
                    data,
                })
            }
            TAG_DONE => Ok(WireMessage::Done),
            TAG_ACCEPT => Ok(WireMessage::Accept),
            TAG_REJECT => Ok(WireMessage::Reject),
            TAG_TEXT => {
                let text = String::from_utf8(rest.to_vec())
                    .map_err(|_| AppError::new("text message was not valid UTF-8"))?;
                Ok(WireMessage::Text { text })
            }
            other => Err(AppError::new(format!("unknown wire message tag {other}"))),
        }
    }
}

async fn send_message(session: &mut NoiseSession, msg: &WireMessage) -> AppResult<()> {
    session.send(&msg.encode()?).await
}

async fn recv_message(session: &mut NoiseSession) -> AppResult<WireMessage> {
    let bytes = session.recv().await?;
    WireMessage::decode(&bytes)
}


pub async fn send_files(
    session: &mut NoiseSession,
    base_dir: &Path,
    paths: &[PathBuf],
    folder_archive_name: Option<String>,
    transfer_id: Option<String>,
    mut on_progress: impl FnMut(SendProgress),
    is_cancelled: impl Fn() -> bool,
    is_paused: impl Fn() -> bool,


    throttle_bytes_per_sec: impl Fn() -> Option<u64>,
) -> AppResult<SendOutcome> {
    let mut manifest = manifest::build_manifest(base_dir, paths)?;
    manifest.folder_archive_name = folder_archive_name;
    let mut wire_manifest = WireTransferManifest::from(&manifest);


    wire_manifest.transfer_id = transfer_id;
    send_message(session, &WireMessage::Manifest(wire_manifest)).await?;

    match recv_message(session).await? {
        WireMessage::Accept => {}
        WireMessage::Reject => return Ok(SendOutcome::Rejected),
        _ => return Err(AppError::new("expected accept/reject response to manifest")),
    }

    let mut progress = SendProgress::default();


    let mut pace_deadline = Instant::now();

    for (file_index, file) in manifest.files.iter().enumerate() {
        let file_index = file_index as u32;

        let missing = match recv_message(session).await? {
            WireMessage::MissingChunks(p) if p.file_index == file_index => p.indices,
            _ => return Err(AppError::new("expected missing-chunks message out of order")),
        };

        let source_path = base_dir.join(&file.relative_path);


        let mut source = std::fs::File::open(&source_path)
            .map_err(|e| AppError::new(format!("failed to open {}: {e}", source_path.display())))?;
        for chunk_index in missing {
            if is_cancelled() {
                return Ok(SendOutcome::Cancelled);
            }


            while is_paused() {
                if is_cancelled() {
                    return Ok(SendOutcome::Cancelled);
                }
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            }

            let local_start = Instant::now();
            let data = chunk::read_chunk_from(&mut source, chunk_index, file.chunk_size)?;
            let hash = *blake3::hash(&data).as_bytes();
            let data_len = data.len() as u64;
            let plaintext = WireMessage::Chunk {
                file_index,
                chunk_index,
                hash,
                data,
            }
            .encode()?;
            progress.local_micros += local_start.elapsed().as_micros() as u64;
            progress.bytes_sent += data_len;


            let mut msg = Vec::with_capacity(4 + plaintext.len());
            msg.extend_from_slice(&(plaintext.len() as u32).to_be_bytes());
            msg.extend_from_slice(&plaintext);

            let encrypt_start = Instant::now();
            let framed = session.seal(&msg)?;
            progress.encrypt_micros += encrypt_start.elapsed().as_micros() as u64;


            let socket_start = Instant::now();
            session.write_sealed(&framed).await?;
            progress.socket_micros += socket_start.elapsed().as_micros() as u64;
            on_progress(progress);


            match throttle_bytes_per_sec().filter(|&r| r > 0) {
                Some(rate) => {
                    let chunk_micros = (data_len as u128 * 1_000_000 / rate as u128) as u64;
                    pace_deadline += std::time::Duration::from_micros(chunk_micros);
                    let now = Instant::now();
                    if now < pace_deadline {
                        tokio::time::sleep(pace_deadline - now).await;
                    } else {


                        pace_deadline = now;
                    }
                }


                None => pace_deadline = Instant::now(),
            }
        }

        send_message(session, &WireMessage::Done).await?;
    }

    Ok(SendOutcome::Completed)
}


pub async fn send_text(session: &mut NoiseSession, text: &str) -> AppResult<()> {
    send_message(session, &WireMessage::Text { text: text.to_string() }).await
}


pub async fn receive_files<F, Fut>(
    session: &mut NoiseSession,
    confirm: F,
    mut on_recv_progress: impl FnMut(RecvProgress),
) -> AppResult<ReceiveOutcome>
where
    F: FnOnce(&WireTransferManifest) -> Fut,
    Fut: std::future::Future<Output = Option<PathBuf>>,
{
    let manifest = match recv_message(session).await? {
        WireMessage::Manifest(m) => m,


        WireMessage::Text { text } => return Ok(ReceiveOutcome::TextMessage(text)),
        _ => return Err(AppError::new("expected manifest message first")),
    };

    let Some(dest_dir) = confirm(&manifest).await else {
        send_message(session, &WireMessage::Reject).await?;
        return Ok(ReceiveOutcome::Rejected);
    };
    let dest_dir = dest_dir.as_path();
    send_message(session, &WireMessage::Accept).await?;

    let mut completed = Vec::new();


    let mut progress = RecvProgress::default();

    for (file_index, file) in manifest.files.iter().enumerate() {
        let file_index = file_index as u32;
        let final_path = dest_dir.join(&file.relative_path);
        let staging_path = {
            let mut p = final_path.as_os_str().to_owned();
            p.push(".part");
            PathBuf::from(p)
        };

        let total_chunks = manifest::chunk_count(file.size, file.chunk_size);
        let mut state = resume::load(&staging_path, &manifest.manifest_id);
        state.manifest_id = manifest.manifest_id.clone();

        let missing: Vec<u32> = (0..total_chunks)
            .filter(|i| !state.verified_chunks.contains(i))
            .collect();

        send_message(
            session,
            &WireMessage::MissingChunks(MissingChunksPayload {
                file_index,
                indices: missing,
            }),
        )
        .await?;


        let staging = chunk::open_staging_file(&staging_path, file.size)?;

        let (tx, rx) = tokio::sync::mpsc::channel::<(u32, [u8; 32], Vec<u8>)>(128);
        let bytes_shared = Arc::new(AtomicU64::new(progress.bytes_written));
        let disk_shared = Arc::new(AtomicU64::new(progress.disk_write_micros));

        let consumer = {
            let bytes_shared = Arc::clone(&bytes_shared);
            let disk_shared = Arc::clone(&disk_shared);
            let staging_path = staging_path.clone();
            let manifest_id = manifest.manifest_id.clone();
            let chunk_size = file.chunk_size;
            let relative_path = file.relative_path.clone();
            let mut verified = state.verified_chunks.clone();
            tokio::task::spawn_blocking(move || -> AppResult<()> {
                let mut staging = staging;
                let mut rx = rx;
                let mut since_save: u32 = 0;
                while let Some((chunk_index, hash, data)) = rx.blocking_recv() {
                    let start = Instant::now();
                    if !integrity::verify_chunk(&data, &hash) {
                        return Err(AppError::new(format!(
                            "chunk {chunk_index} of {relative_path} failed integrity check"
                        )));
                    }
                    chunk::write_chunk_to(&mut staging, chunk_index, chunk_size, &data)?;
                    disk_shared.fetch_add(start.elapsed().as_micros() as u64, Ordering::Relaxed);
                    bytes_shared.fetch_add(data.len() as u64, Ordering::Relaxed);
                    verified.insert(chunk_index);
                    since_save += 1;
                    if since_save >= 32 {
                        resume::save(
                            &staging_path,
                            &resume::ResumeState {
                                manifest_id: manifest_id.clone(),
                                verified_chunks: verified.clone(),
                            },
                        )?;
                        since_save = 0;
                    }
                }
                if since_save > 0 {
                    resume::save(
                        &staging_path,
                        &resume::ResumeState {
                            manifest_id,
                            verified_chunks: verified,
                        },
                    )?;
                }
                Ok(())
            })
        };

        let recv_result: AppResult<()> = loop {
            let recv_start = Instant::now();
            let message = match recv_message(session).await {
                Ok(m) => m,
                Err(e) => break Err(e),
            };
            progress.recv_wait_micros += recv_start.elapsed().as_micros() as u64;
            match message {
                WireMessage::Chunk {
                    file_index: fi,
                    chunk_index,
                    hash,
                    data,
                } if fi == file_index => {
                    if tx.send((chunk_index, hash, data)).await.is_err() {
                        break Ok(());
                    }
                    progress.bytes_written = bytes_shared.load(Ordering::Relaxed);
                    progress.disk_write_micros = disk_shared.load(Ordering::Relaxed);
                    on_recv_progress(progress);
                }
                WireMessage::Done => break Ok(()),
                _ => break Err(AppError::new("unexpected message while receiving chunks")),
            }
        };

        drop(tx);
        consumer
            .await
            .map_err(|e| AppError::new(format!("write task join failed: {e}")))??;
        recv_result?;

        progress.bytes_written = bytes_shared.load(Ordering::Relaxed);
        progress.disk_write_micros = disk_shared.load(Ordering::Relaxed);
        on_recv_progress(progress);

        if !integrity::verify_file(&staging_path, &file.root_hash)? {
            return Err(AppError::new(format!(
                "{} failed whole-file integrity check after all chunks received",
                file.relative_path
            )));
        }

        std::fs::rename(&staging_path, &final_path)
            .map_err(|e| AppError::new(format!("failed to finalize {}: {e}", final_path.display())))?;
        resume::clear(&staging_path);

        completed.push(final_path);
    }

    Ok(ReceiveOutcome::Completed {
        files: completed,
        folder_archive_name: manifest.folder_archive_name,
        total_bytes: manifest.files.iter().map(|f| f.size).sum(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::endpoint;
    use crate::transport::session::{upgrade_inbound_stream, upgrade_outbound};

    async fn connected_pair() -> (NoiseSession, NoiseSession, quinn::Connection, quinn::Connection)
    {
        let server_ep = endpoint::bind(0).unwrap();
        let server_port = server_ep.local_addr().unwrap().port();
        let server_addr: std::net::SocketAddr = ([127, 0, 0, 1], server_port).into();
        let client_ep = endpoint::bind(0).unwrap();

        let psk = [9u8; 32];

        let server_side = tokio::spawn(async move {
            let incoming = server_ep.accept().await.unwrap();
            let connection = incoming.await.unwrap();
            let (send, recv) = connection.accept_bi().await.unwrap();
            let session = upgrade_inbound_stream(send, recv, &psk).await.unwrap();
            (session, connection)
        });

        let client_connection = client_ep
            .connect(server_addr, "zwormhole.local")
            .unwrap()
            .await
            .unwrap();
        let client_session = upgrade_outbound(&client_connection, &psk).await.unwrap();

        let (server_session, server_connection) = server_side.await.unwrap();

        (
            client_session,
            server_session,
            client_connection,
            server_connection,
        )
    }

    #[tokio::test]
    async fn transfers_a_file_end_to_end() {
        let (mut sender, mut receiver, _c1, _c2) = connected_pair().await;

        let src_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();

        let content = vec![42u8; manifest::CHUNK_SIZE as usize * 3 + 500];
        let src_path = src_dir.path().join("payload.bin");
        std::fs::write(&src_path, &content).unwrap();

        let send_task = {
            let src_dir = src_dir.path().to_path_buf();
            let src_path = src_path.clone();
            tokio::spawn(async move {
                let outcome = send_files(&mut sender, &src_dir, &[src_path], None, None, |_| {}, || false, || false, || None)
                    .await
                    .unwrap();
                assert_eq!(outcome, SendOutcome::Completed);
            })
        };

        let dest = dest_dir.path().to_path_buf();
        let outcome = receive_files(&mut receiver, |_manifest| async { Some(dest) }, |_| {})
            .await
            .unwrap();
        send_task.await.unwrap();

        let ReceiveOutcome::Completed { files: received, .. } = outcome else {
            panic!("expected the transfer to complete, not be rejected");
        };
        assert_eq!(received.len(), 1);
        let received_content = std::fs::read(&received[0]).unwrap();
        assert_eq!(received_content, content);
    }

    #[tokio::test]
    async fn recipient_can_reject_before_any_bytes_are_written() {
        let (mut sender, mut receiver, _c1, _c2) = connected_pair().await;

        let src_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();

        let src_path = src_dir.path().join("payload.bin");
        std::fs::write(&src_path, b"unsolicited content").unwrap();

        let send_task = {
            let src_dir = src_dir.path().to_path_buf();
            let src_path = src_path.clone();
            tokio::spawn(async move {
                let outcome = send_files(&mut sender, &src_dir, &[src_path], None, None, |_| {}, || false, || false, || None)
                    .await
                    .unwrap();
                assert_eq!(
                    outcome,
                    SendOutcome::Rejected,
                    "sender should observe the rejection"
                );
            })
        };

        let outcome = receive_files(&mut receiver, |_manifest| async { None }, |_| {})
            .await
            .unwrap();
        send_task.await.unwrap();

        assert!(matches!(outcome, ReceiveOutcome::Rejected));
        assert!(
            std::fs::read_dir(dest_dir.path()).unwrap().next().is_none(),
            "nothing should be written to disk for a rejected transfer"
        );
    }

    #[tokio::test]
    async fn cancelling_stops_the_send_early() {
        let (mut sender, mut receiver, _c1, _c2) = connected_pair().await;

        let src_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();

        let content = vec![9u8; manifest::CHUNK_SIZE as usize * 4];
        let src_path = src_dir.path().join("payload.bin");
        std::fs::write(&src_path, &content).unwrap();


        let dest = dest_dir.path().to_path_buf();
        let recv_task = tokio::spawn(async move {
            let _ = receive_files(&mut receiver, |_m| async { Some(dest) }, |_| {}).await;
            receiver
        });


        let src = src_dir.path().to_path_buf();
        let outcome = send_files(&mut sender, &src, &[src_path], None, None, |_| {}, || true, || false, || None)
            .await
            .unwrap();
        assert_eq!(outcome, SendOutcome::Cancelled);

        drop(sender);
        let _ = recv_task.await;

        assert!(
            std::fs::read_dir(dest_dir.path())
                .unwrap()
                .all(|e| e.unwrap().path().extension().is_some_and(|x| x == "part")),
            "a cancelled transfer leaves at most a .part staging file, never a finished file"
        );
    }

    #[tokio::test]
    async fn pausing_parks_the_send_then_resume_completes_it() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let (mut sender, mut receiver, _c1, _c2) = connected_pair().await;
        let src_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();

        let content = vec![7u8; manifest::CHUNK_SIZE as usize * 4];
        let src_path = src_dir.path().join("payload.bin");
        std::fs::write(&src_path, &content).unwrap();

        let dest = dest_dir.path().to_path_buf();
        let recv_task = tokio::spawn(async move {
            receive_files(&mut receiver, |_m| async { Some(dest) }, |_| {}).await
        });


        let paused = Arc::new(AtomicBool::new(true));
        let paused_flip = paused.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            paused_flip.store(false, Ordering::SeqCst);
        });

        let src = src_dir.path().to_path_buf();
        let paused_check = paused.clone();
        let started = std::time::Instant::now();
        let outcome = send_files(
            &mut sender,
            &src,
            &[src_path],
            None,
            None,
            |_| {},
            || false,
            move || paused_check.load(Ordering::SeqCst),
            || None,
        )
        .await
        .unwrap();

        assert_eq!(outcome, SendOutcome::Completed);
        assert!(
            started.elapsed() >= std::time::Duration::from_millis(350),
            "the send should have parked while paused, not completed immediately"
        );

        let recv = recv_task.await.unwrap().unwrap();
        match recv {
            ReceiveOutcome::Completed { total_bytes, .. } => {
                assert_eq!(total_bytes, content.len() as u64);
            }
            _ => panic!("expected a completed receive"),
        }
    }

    #[tokio::test]
    async fn bandwidth_cap_paces_the_send() {
        let (mut sender, mut receiver, _c1, _c2) = connected_pair().await;
        let src_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();

        let content = vec![5u8; manifest::CHUNK_SIZE as usize * 4];
        let src_path = src_dir.path().join("payload.bin");
        std::fs::write(&src_path, &content).unwrap();

        let dest = dest_dir.path().to_path_buf();
        let recv_task = tokio::spawn(async move {
            receive_files(&mut receiver, |_m| async { Some(dest) }, |_| {}).await
        });


        let rate = (content.len() as u64 * 10) / 4;
        let src = src_dir.path().to_path_buf();
        let started = std::time::Instant::now();
        let outcome = send_files(
            &mut sender,
            &src,
            &[src_path],
            None,
            None,
            |_| {},
            || false,
            || false,
            move || Some(rate),
        )
        .await
        .unwrap();
        assert_eq!(outcome, SendOutcome::Completed);
        assert!(
            started.elapsed() >= std::time::Duration::from_millis(300),
            "a rate-capped send should be paced, not run at full speed (took {:?})",
            started.elapsed()
        );
        let _ = recv_task.await;
    }

    #[tokio::test]
    async fn round_trips_transfer_id_and_reports_receiver_timing() {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::{Arc, Mutex};

        let (mut sender, mut receiver, _c1, _c2) = connected_pair().await;
        let src_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();

        let content = vec![3u8; manifest::CHUNK_SIZE as usize * 3];
        let src_path = src_dir.path().join("payload.bin");
        std::fs::write(&src_path, &content).unwrap();


        let seen_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let recv_bytes = Arc::new(AtomicU64::new(0));
        let dest = dest_dir.path().to_path_buf();
        let seen_id_c = seen_id.clone();
        let recv_bytes_c = recv_bytes.clone();
        let recv_task = tokio::spawn(async move {
            let confirm = move |m: &WireTransferManifest| {
                *seen_id_c.lock().unwrap() = m.transfer_id.clone();
                async move { Some(dest) }
            };
            receive_files(&mut receiver, confirm, |p: RecvProgress| {
                recv_bytes_c.store(p.bytes_written, Ordering::Relaxed);
            })
            .await
            .unwrap()
        });

        let src = src_dir.path().to_path_buf();
        let outcome = send_files(
            &mut sender,
            &src,
            &[src_path],
            None,
            Some("job-abc123".to_string()),
            |_| {},
            || false,
            || false,
            || None,
        )
        .await
        .unwrap();
        assert_eq!(outcome, SendOutcome::Completed);
        recv_task.await.unwrap();

        assert_eq!(seen_id.lock().unwrap().as_deref(), Some("job-abc123"));
        assert_eq!(recv_bytes.load(Ordering::Relaxed), content.len() as u64);
    }

    #[tokio::test]
    async fn resumes_only_missing_chunks() {
        let (mut sender, mut receiver, _c1, _c2) = connected_pair().await;

        let src_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();

        let content = vec![7u8; manifest::CHUNK_SIZE as usize * 2];
        let src_path = src_dir.path().join("payload.bin");
        std::fs::write(&src_path, &content).unwrap();


        let built = manifest::build_manifest(src_dir.path(), &[src_path.clone()]).unwrap();
        let final_path = dest_dir.path().join("payload.bin");
        let staging_path = {
            let mut p = final_path.as_os_str().to_owned();
            p.push(".part");
            PathBuf::from(p)
        };
        chunk::write_chunk_at(
            &staging_path,
            0,
            manifest::CHUNK_SIZE,
            built.files[0].size,
            &content[..manifest::CHUNK_SIZE as usize],
        )
        .unwrap();
        let mut state = resume::ResumeState {
            manifest_id: built.manifest_id.clone(),
            verified_chunks: Default::default(),
        };
        state.verified_chunks.insert(0);
        resume::save(&staging_path, &state).unwrap();

        let send_task = {
            let src_dir = src_dir.path().to_path_buf();
            let src_path = src_path.clone();
            tokio::spawn(async move {
                send_files(&mut sender, &src_dir, &[src_path], None, None, |_| {}, || false, || false, || None)
                    .await
                    .unwrap();
            })
        };

        let dest = dest_dir.path().to_path_buf();
        let outcome = receive_files(&mut receiver, |_manifest| async { Some(dest) }, |_| {})
            .await
            .unwrap();
        send_task.await.unwrap();

        let ReceiveOutcome::Completed { files: received, .. } = outcome else {
            panic!("expected the transfer to complete, not be rejected");
        };
        assert_eq!(std::fs::read(&received[0]).unwrap(), content);
    }
}
