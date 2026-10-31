

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use quinn::{Connection, Endpoint, RecvStream, SendStream};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::error::{AppError, AppResult};
use crate::identity::DeviceIdentity;
use crate::pairing::spake::PairingExchange;
use crate::settings::store::{AutoExtractMode, SettingsStore};
use crate::state::AppState;
use crate::transfer::manifest::WireTransferManifest;
use crate::transfer::protocol::{self, ReceiveOutcome};
use crate::transfer::archive;
use crate::profile::{MyProfileStore, ProfileWire, RemoteProfileStore};
use crate::transport::{endpoint as transport_endpoint, session};
use crate::trusted::store::TrustedDevice;


pub async fn ensure_endpoint(app: &AppHandle) -> AppResult<Endpoint> {
    let state = app.state::<AppState>();
    let mut net = state.net.lock().await;

    if let Some(endpoint) = &net.endpoint {
        return Ok(endpoint.clone());
    }


    let configured_port = app
        .try_state::<SettingsStore>()
        .map(|s| s.quic_port())
        .unwrap_or(0);
    let endpoint = if configured_port != 0 {
        transport_endpoint::bind(configured_port)?
    } else {
        transport_endpoint::bind_default()?
    };
    net.endpoint = Some(endpoint.clone());
    drop(net);

    spawn_incoming_listener(app.clone(), endpoint.clone());

    Ok(endpoint)
}


pub async fn reset_endpoint(app: &AppHandle) {
    let state = app.state::<AppState>();
    let old = {
        let mut net = state.net.lock().await;
        net.endpoint.take()
    };
    if let Some(endpoint) = old {
        endpoint.close(0u32.into(), b"rebinding");
    }
}

const PAIRING_REQUESTED: &str = "zw://pairing/requested";
const PAIRING_SUCCEEDED: &str = "zw://pairing/succeeded";
const PAIRING_FAILED: &str = "zw://pairing/failed";
pub const TRANSFER_STARTED: &str = "zw://transfer/started";
pub const TRANSFER_ARCHIVING: &str = "zw://transfer/archiving";


pub const TRANSFER_PROGRESS: &str = "zw://transfer/progress";
pub const TRANSFER_COMPLETED: &str = "zw://transfer/completed";


pub const RECEIVE_COMPLETED: &str = "zw://transfer/received";


pub const RECEIVE_PROGRESS: &str = "zw://transfer/receive-progress";
pub const TRANSFER_FAILED: &str = "zw://transfer/failed";


pub const TRANSFER_CANCELLED: &str = "zw://transfer/cancelled";
pub const TRANSFER_REJECTED: &str = "zw://transfer/rejected";
pub const PEER_DISCONNECTED: &str = "zw://pairing/peer-disconnected";
pub const INCOMING_TRANSFER_REQUEST: &str = "zw://transfer/incoming-request";
pub const INCOMING_TRANSFER_EXPIRED: &str = "zw://transfer/incoming-expired";


pub const TEXT_RECEIVED: &str = "zw://text/received";


pub const PROFILE_UPDATED: &str = "zw://profile/updated";

#[derive(Debug, Clone, Serialize)]
pub struct TextReceivedPayload {
    pub device_key: String,
    pub display_name: String,
    pub text: String,
}


const INCOMING_TRANSFER_TIMEOUT: Duration = Duration::from_secs(120);


#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
enum PairingStreamRequest {
    FastReconnect {
        fingerprint: String,


        #[serde(default)]
        profile: Option<ProfileWire>,
    },
    Full {
        session_identity: String,
        requester_name: String,
        requester_fingerprint: String,
        code: String,
        spake_msg: Vec<u8>,


        expiry_secs: Option<u64>,
        #[serde(default)]
        requester_profile: Option<ProfileWire>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
enum PairingStreamResponse {
    FastReconnectAccepted {


        #[serde(default)]
        profile: Option<ProfileWire>,
    },
    FastReconnectRejected,
    FullAccepted {
        spake_msg: Vec<u8>,


        #[serde(default)]
        responder_fingerprint: String,
        #[serde(default)]
        responder_name: String,
        #[serde(default)]
        responder_profile: Option<ProfileWire>,
    },
    FullRejected,
}

async fn write_json_frame<T: Serialize>(send: &mut SendStream, value: &T) -> AppResult<()> {
    let bytes = serde_json::to_vec(value)
        .map_err(|e| AppError::new(format!("failed to encode pairing message: {e}")))?;
    let len = u32::try_from(bytes.len())
        .map_err(|_| AppError::new("pairing message too large"))?;
    send.write_all(&len.to_be_bytes())
        .await
        .map_err(|e| AppError::new(format!("failed to write pairing frame: {e}")))?;
    send.write_all(&bytes)
        .await
        .map_err(|e| AppError::new(format!("failed to write pairing frame: {e}")))
}

async fn read_json_frame<T: for<'de> Deserialize<'de>>(recv: &mut RecvStream) -> AppResult<T> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf)
        .await
        .map_err(|e| AppError::new(format!("failed to read pairing frame length: {e}")))?;
    let len = u32::from_be_bytes(len_buf) as usize;

    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf)
        .await
        .map_err(|e| AppError::new(format!("failed to read pairing frame: {e}")))?;
    serde_json::from_slice(&buf)
        .map_err(|e| AppError::new(format!("failed to decode pairing message: {e}")))
}

fn downloads_dir(app: &AppHandle) -> AppResult<PathBuf> {
    crate::platform::default_save_dir(app)
}

pub fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}


fn notify_arrival(app: &AppHandle, title: &str, body: &str) {
    let focused = app
        .get_webview_window("main")
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false);
    if focused {
        return;
    }
    let _ = app.notification().builder().title(title).body(body).show();
}


#[derive(Debug, Clone, Serialize)]
pub struct ReceiveCompletedPayload {


    pub id: String,
    pub device_key: String,
    pub display_name: String,
    pub file_names: Vec<String>,
    pub paths: Vec<String>,
}


#[derive(Debug, Clone, Serialize)]
pub struct ReceiveProgressPayload {
    pub id: String,
    pub device_key: String,
    pub display_name: String,

    pub label: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
}


fn label_from_names(names: &[String]) -> String {
    match names.split_first() {
        Some((first, rest)) if rest.is_empty() => first.clone(),
        Some((first, rest)) => format!("{first} +{}", rest.len()),
        None => "files".to_string(),
    }
}


pub struct IncomingDecision {
    pub accept: bool,
    pub save_dir: Option<PathBuf>,
    pub extract_mode: Option<AutoExtractMode>,
}


#[derive(Debug, Clone, Serialize)]
pub struct IncomingFileInfo {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncomingTransferRequestPayload {
    pub request_id: String,
    pub device_key: String,


    pub display_name: String,
    pub file_names: Vec<String>,

    pub files: Vec<IncomingFileInfo>,
    pub total_size: u64,
    pub default_save_dir: String,


    pub folder_archive_name: Option<String>,


    pub default_extract_mode: AutoExtractMode,
}


async fn confirm_incoming_transfer(
    app: &AppHandle,
    device_key: &str,
    manifest: &WireTransferManifest,
) -> Option<(PathBuf, Option<AutoExtractMode>)> {


    let total_size: u64 = manifest.files.iter().map(|f| f.size).sum();


    let files: Vec<IncomingFileInfo> = if let (Some(folder_name), [_single]) =
        (&manifest.folder_archive_name, manifest.files.as_slice())
    {
        vec![IncomingFileInfo {
            name: format!("{folder_name}/ (folder)"),
            size: total_size,
        }]
    } else {
        manifest
            .files
            .iter()
            .map(|f| IncomingFileInfo {
                name: f.relative_path.clone(),
                size: f.size,
            })
            .collect()
    };
    let file_names: Vec<String> = files.iter().map(|f| f.name.clone()).collect();

    let display_name = {
        let state = app.state::<AppState>();
        let net = state.net.lock().await;
        net.peers
            .get(device_key)
            .map(|p| p.display_name.clone())
            .unwrap_or_else(|| device_key.to_string())
    };

    let settings = app.try_state::<SettingsStore>().map(|s| s.get());
    let default_extract_mode = settings.as_ref().map(|s| s.auto_extract).unwrap_or_default();


    let device_auto_accept = app
        .try_state::<crate::trusted::store::TrustedStore>()
        .and_then(|s| s.get(device_key).ok().flatten())
        .map(|d| d.auto_accept)
        .unwrap_or(false);
    let global_auto_accept = settings.as_ref().map(|s| s.auto_accept).unwrap_or(false);

    if global_auto_accept || device_auto_accept {


        let dir = settings
            .as_ref()
            .and_then(|s| s.save_dir.clone())
            .or_else(|| downloads_dir(app).ok())?;
        return Some((dir, None));
    }

    let default_save_dir = settings
        .and_then(|s| s.save_dir)
        .or_else(|| downloads_dir(app).ok())?;

    let request_id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel();
    {
        let state = app.state::<AppState>();
        let mut net = state.net.lock().await;
        net.pending_incoming.insert(request_id.clone(), tx);
    }

    notify_arrival(
        app,
        "Incoming files",
        &format!(
            "{display_name} wants to send you {} file{}",
            file_names.len(),
            if file_names.len() == 1 { "" } else { "s" }
        ),
    );
    let _ = app.emit(
        INCOMING_TRANSFER_REQUEST,
        IncomingTransferRequestPayload {
            request_id: request_id.clone(),
            device_key: device_key.to_string(),
            display_name,
            file_names,
            files,
            total_size,
            default_save_dir: default_save_dir.to_string_lossy().to_string(),
            folder_archive_name: manifest.folder_archive_name.clone(),
            default_extract_mode,
        },
    );

    let decision = tokio::time::timeout(INCOMING_TRANSFER_TIMEOUT, rx).await;

    {
        let state = app.state::<AppState>();
        let mut net = state.net.lock().await;
        net.pending_incoming.remove(&request_id);
    }

    match decision {
        Ok(Ok(d)) if d.accept => {
            Some((d.save_dir.unwrap_or(default_save_dir), d.extract_mode))
        }
        Ok(Ok(_)) => None,
        Ok(Err(_)) => None,
        Err(_) => {


            let _ = app.emit(INCOMING_TRANSFER_EXPIRED, &request_id);
            None
        }
    }
}


pub fn spawn_transfer_acceptor(
    app: AppHandle,
    connection: Connection,
    secret: [u8; 32],
    device_key: String,
) {
    tokio::spawn(async move {
        loop {
            let (send, recv) = match connection.accept_bi().await {
                Ok(pair) => pair,
                Err(_) => break,
            };

            let app = app.clone();
            let device_key = device_key.clone();
            let connection_for_stats = connection.clone();
            tokio::spawn(async move {
                let mut session = match session::upgrade_inbound_stream(send, recv, &secret).await
                {
                    Ok(s) => s,
                    Err(_) => return,
                };


                let transfer_id_slot: Arc<std::sync::Mutex<Option<String>>> =
                    Arc::new(std::sync::Mutex::new(None));
                let (stats_tx, mut stats_rx) = mpsc::unbounded_channel::<protocol::RecvProgress>();
                {
                    let connection = connection_for_stats;
                    let transfer_id_slot = transfer_id_slot.clone();
                    tokio::spawn(async move {
                        let mut last = Instant::now();
                        let mut pending: Option<protocol::RecvProgress> = None;
                        let mut open = true;
                        while open {


                            let flush = match stats_rx.recv().await {
                                Some(p) => {
                                    let mut latest = p;
                                    while let Ok(next) = stats_rx.try_recv() {
                                        latest = next;
                                    }
                                    pending = Some(latest);
                                    last.elapsed() >= TRANSFER_STATS_INTERVAL
                                }
                                None => {
                                    open = false;
                                    true
                                }
                            };
                            if !flush {
                                continue;
                            }
                            let Some(p) = pending.take() else { continue };
                            let Some(transfer_id) = transfer_id_slot.lock().unwrap().clone() else {
                                continue;
                            };
                            if let Ok(mut send) = connection.open_uni().await {
                                let _ = write_uni_frame(
                                    &mut send,
                                    &UniStreamMessage::TransferStats {
                                        transfer_id,
                                        recv_wait_micros: p.recv_wait_micros,
                                        disk_write_micros: p.disk_write_micros,
                                        bytes: p.bytes_written,
                                    },
                                )
                                .await;
                                let _ = send.finish();
                            }
                            last = Instant::now();
                        }
                    });
                }


                let recv_id = uuid::Uuid::new_v4().to_string();
                let recv_meta: Arc<std::sync::Mutex<Option<(String, u64)>>> =
                    Arc::new(std::sync::Mutex::new(None));
                let recv_meta_confirm = recv_meta.clone();
                let last_recv_emit: Arc<std::sync::Mutex<Instant>> = Arc::new(std::sync::Mutex::new(
                    Instant::now()
                        .checked_sub(RECEIVE_PROGRESS_INTERVAL)
                        .unwrap_or_else(Instant::now),
                ));


                let recv_curve: Arc<std::sync::Mutex<Vec<u32>>> =
                    Arc::new(std::sync::Mutex::new(Vec::new()));
                let recv_last: Arc<std::sync::Mutex<Option<(u64, Instant)>>> =
                    Arc::new(std::sync::Mutex::new(None));

                let device_key_for_confirm = device_key.clone();
                let app_for_confirm = app.clone();
                let transfer_id_for_confirm = transfer_id_slot.clone();


                let chosen_extract_mode: Arc<Mutex<Option<AutoExtractMode>>> =
                    Arc::new(Mutex::new(None));
                let chosen_extract_mode_for_confirm = chosen_extract_mode.clone();
                let confirm = move |manifest: &WireTransferManifest| {


                    *transfer_id_for_confirm.lock().unwrap() = manifest.transfer_id.clone();


                    let names: Vec<String> = manifest
                        .files
                        .iter()
                        .map(|f| {
                            f.relative_path
                                .rsplit(['/', '\\'])
                                .next()
                                .unwrap_or(&f.relative_path)
                                .to_string()
                        })
                        .collect();
                    let total: u64 = manifest.files.iter().map(|f| f.size).sum();
                    *recv_meta_confirm.lock().unwrap() = Some((label_from_names(&names), total));
                    let app = app_for_confirm.clone();
                    let device_key = device_key_for_confirm.clone();
                    let manifest = manifest.clone();
                    let chosen_extract_mode = chosen_extract_mode_for_confirm.clone();
                    async move {
                        let (dest_dir, mode) =
                            confirm_incoming_transfer(&app, &device_key, &manifest).await?;
                        *chosen_extract_mode.lock().await = mode;
                        Some(dest_dir)
                    }
                };

                let app_for_recv = app.clone();
                let device_key_for_recv = device_key.clone();
                let recv_id_for_recv = recv_id.clone();
                let recv_meta_progress = recv_meta.clone();
                let last_recv_emit_progress = last_recv_emit.clone();
                let recv_curve_progress = recv_curve.clone();
                let recv_last_progress = recv_last.clone();
                let on_recv_progress = move |p: protocol::RecvProgress| {
                    let bytes_done = p.bytes_written;


                    let _ = stats_tx.send(p);


                    let Some((label, total)) = recv_meta_progress.lock().unwrap().clone() else {
                        return;
                    };
                    let mut last = last_recv_emit_progress.lock().unwrap();
                    if last.elapsed() < RECEIVE_PROGRESS_INTERVAL {
                        return;
                    }
                    let now = Instant::now();
                    *last = now;
                    drop(last);


                    {
                        let mut prev = recv_last_progress.lock().unwrap();
                        if let Some((pb, pt)) = *prev {
                            let dt = now.duration_since(pt).as_secs_f64();
                            if dt > 0.0 && bytes_done >= pb {
                                let rate = ((bytes_done - pb) as f64 / dt) as u32;
                                recv_curve_progress.lock().unwrap().push(rate);
                            }
                        }
                        *prev = Some((bytes_done, now));
                    }
                    let display_name = app_for_recv
                        .try_state::<crate::trusted::store::TrustedStore>()
                        .and_then(|s| s.get(&device_key_for_recv).ok().flatten())
                        .map(|d| d.display_name)
                        .unwrap_or_else(|| device_key_for_recv.clone());
                    let _ = app_for_recv.emit(
                        RECEIVE_PROGRESS,
                        ReceiveProgressPayload {
                            id: recv_id_for_recv.clone(),
                            device_key: device_key_for_recv.clone(),
                            display_name,
                            label,
                            bytes_done,
                            bytes_total: total,
                        },
                    );
                };

                match protocol::receive_files(&mut session, confirm, on_recv_progress).await {
                    Ok(ReceiveOutcome::Completed {
                        files,
                        folder_archive_name,
                        total_bytes,
                    }) => {


                        let files = if let (Some(folder_name), [zip_path]) =
                            (folder_archive_name, files.as_slice())
                        {
                            let mode = chosen_extract_mode.lock().await.unwrap_or_else(|| {
                                app.try_state::<SettingsStore>()
                                    .map(|s| s.get().auto_extract)
                                    .unwrap_or_default()
                            });
                            match mode {
                                AutoExtractMode::ExtractAndDelete
                                | AutoExtractMode::ExtractAndKeep => {
                                    let dest_dir = zip_path
                                        .parent()
                                        .map(PathBuf::from)
                                        .unwrap_or_else(PathBuf::new);
                                    match archive::extract_zip(zip_path, &dest_dir, &folder_name) {
                                        Ok(extracted) => {
                                            if mode == AutoExtractMode::ExtractAndDelete {
                                                let _ = std::fs::remove_file(zip_path);
                                            }
                                            vec![extracted]
                                        }
                                        Err(_) => files,
                                    }
                                }
                                AutoExtractMode::Off => {


                                    let dest_dir = zip_path
                                        .parent()
                                        .map(PathBuf::from)
                                        .unwrap_or_else(PathBuf::new);
                                    match archive::rename_to_clean_zip_name(
                                        zip_path,
                                        &dest_dir,
                                        &folder_name,
                                    ) {
                                        Ok(renamed) => vec![renamed],
                                        Err(_) => files,
                                    }
                                }
                            }
                        } else {
                            files
                        };

                        crate::platform::files_saved(&app, &files);
                        let names = files
                            .iter()
                            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                            .collect::<Vec<_>>();
                        let paths = files
                            .iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect::<Vec<_>>();
                        record_transfer_history(
                            &app,
                            &device_key,
                            names.clone(),
                            "received",
                            total_bytes,
                            paths.clone(),
                            downsample(&recv_curve.lock().unwrap(), 60),
                        );
                        let who = app
                            .try_state::<crate::trusted::store::TrustedStore>()
                            .and_then(|s| s.get(&device_key).ok().flatten())
                            .map(|d| d.display_name)
                            .unwrap_or_else(|| device_key.clone());
                        notify_arrival(
                            &app,
                            "Transfer complete",
                            &format!(
                                "Received {} file{} from {who}",
                                names.len(),
                                if names.len() == 1 { "" } else { "s" }
                            ),
                        );
                        let _ = app.emit(
                            RECEIVE_COMPLETED,
                            ReceiveCompletedPayload {
                                id: recv_id,
                                device_key,
                                display_name: who,
                                file_names: names,
                                paths,
                            },
                        );
                    }
                    Ok(ReceiveOutcome::Rejected) => {
                        let _ = app.emit(TRANSFER_REJECTED, &device_key);
                    }
                    Ok(ReceiveOutcome::TextMessage(text)) => {
                        let display_name = app
                            .try_state::<crate::trusted::store::TrustedStore>()
                            .and_then(|s| s.get(&device_key).ok().flatten())
                            .map(|d| d.display_name)
                            .unwrap_or_else(|| device_key.clone());
                        notify_arrival(&app, "Message received", &format!("{display_name} sent you text"));
                        let _ = app.emit(
                            TEXT_RECEIVED,
                            TextReceivedPayload {
                                device_key: device_key.clone(),
                                display_name,
                                text,
                            },
                        );
                    }
                    Err(e) => {
                        let _ = app.emit(TRANSFER_FAILED, format!("{device_key}: {e}"));
                    }
                }
            });
        }


        let state = app.state::<AppState>();
        {
            let mut net = state.net.lock().await;
            net.peers.remove(&device_key);
        }
        let _ = app.emit(PEER_DISCONNECTED, &device_key);
    });
}


fn log_key_agreement(role: &str, session_identity: &str, secret: &crate::pairing::spake::SharedSecret) {
    eprintln!(
        "[zwormhole] pairing {role} session={session_identity} key_fingerprint={}",
        secret.debug_fingerprint()
    );
}


pub async fn disconnect_peer(app: &AppHandle, device_key: &str) {
    let state = app.state::<AppState>();
    let removed = {
        let mut net = state.net.lock().await;
        net.peers.remove(device_key)
    };
    if let Some(peer) = removed {
        peer.connection.close(0u32.into(), b"revoked");
    }

    {
        let mut net = state.net.lock().await;
        net.test_allowances.remove(device_key);
    }
    let _ = app.emit(PEER_DISCONNECTED, device_key);
}


pub async fn send_text_message(app: &AppHandle, device_key: &str, text: String) -> AppResult<()> {
    let (connection, secret) = {
        let state = app.state::<AppState>();
        let net = state.net.lock().await;
        let peer = net
            .peers
            .get(device_key)
            .ok_or_else(|| AppError::new("not connected to this device — pair first"))?;


        if let Some(reason) = peer.connection.close_reason() {
            return Err(AppError::new(format!(
                "device is no longer connected ({reason}) — reconnect and try again"
            )));
        }
        (peer.connection.clone(), peer.secret)
    };
    let mut session = session::upgrade_outbound(&connection, &secret).await?;
    protocol::send_text(&mut session, &text).await
}


pub fn record_transfer_history(
    app: &AppHandle,
    device_key: &str,
    file_names: Vec<String>,
    direction: &str,
    total_bytes: u64,
    paths: Vec<String>,
    rate_samples: Vec<u32>,
) {
    let display_name = app
        .try_state::<crate::trusted::store::TrustedStore>()
        .and_then(|s| s.get(device_key).ok().flatten())
        .map(|d| d.display_name)
        .unwrap_or_else(|| device_key.to_string());
    if let Some(store) = app.try_state::<crate::history::store::HistoryStore>() {
        let _ = store.record(crate::history::store::HistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            device_key: device_key.to_string(),
            display_name,
            file_names,
            direction: direction.to_string(),
            total_bytes,
            at_unix: now_unix_secs(),
            paths,
            rate_samples,
        });
    }
}


fn downsample(samples: &[u32], max: usize) -> Vec<u32> {
    if samples.len() <= max {
        return samples.to_vec();
    }
    (0..max)
        .map(|i| samples[i * samples.len() / max])
        .collect()
}

fn chrono_now() -> String {


    format!("{}", now_unix_secs())
}


pub fn spawn_incoming_listener(app: AppHandle, endpoint: Endpoint) {
    tokio::spawn(async move {
        loop {
            let incoming = match endpoint.accept().await {
                Some(i) => i,
                None => break,
            };

            let app = app.clone();
            tokio::spawn(async move {
                let connection = match incoming.await {
                    Ok(c) => c,
                    Err(_) => return,
                };
                if let Err(_e) = handle_incoming_pairing_stream(app, connection).await {


                }
            });
        }
    });
}

async fn handle_incoming_pairing_stream(app: AppHandle, connection: Connection) -> AppResult<()> {
    let (send, mut recv) = connection
        .accept_bi()
        .await
        .map_err(|e| AppError::new(format!("failed to accept pairing stream: {e}")))?;

    let request: PairingStreamRequest = read_json_frame(&mut recv).await?;

    match request {
        PairingStreamRequest::FastReconnect { fingerprint, profile } => {
            handle_fast_reconnect_attempt(app, connection, send, fingerprint, profile).await
        }
        PairingStreamRequest::Full {
            session_identity,
            requester_name,
            requester_fingerprint,
            code,
            spake_msg,
            expiry_secs,
            requester_profile,
        } => {


            if is_blocked(&app, &requester_fingerprint) {
                connection.close(0u32.into(), b"blocked");
                return Ok(());
            }

            let request_id = session_identity.clone();
            let code_for_display = code.clone();
            let fingerprint_for_display = requester_fingerprint.clone();

            let state = app.state::<AppState>();
            {
                let mut net = state.net.lock().await;
                net.inbound_pending.insert(
                    request_id.clone(),
                    PendingInboundRequest {
                        connection,
                        send,
                        session_identity: session_identity.clone(),
                        requester_name: requester_name.clone(),
                        requester_fingerprint,
                        code,
                        requester_msg: spake_msg,
                        expiry_secs,
                        requester_profile,
                    },
                );
            }

            notify_arrival(
                &app,
                "Pairing request",
                &format!("{requester_name} wants to pair with this device"),
            );
            let _ = app.emit(
                PAIRING_REQUESTED,
                PairingRequestedPayload {
                    request_id,
                    requester_name,
                    code: code_for_display,
                    requester_fingerprint: fingerprint_for_display,
                },
            );

            Ok(())
        }
    }
}


fn my_profile_wire(app: &AppHandle) -> ProfileWire {
    let fallback = crate::commands::discovery::local_display_name();
    app.try_state::<MyProfileStore>()
        .map(|s| s.wire(&fallback))
        .unwrap_or(ProfileWire {
            display_name: fallback,
            avatar_png: None,
            updated_at: 0,
        })
}


fn cache_peer_profile(app: &AppHandle, fingerprint: &str, profile: Option<ProfileWire>) {
    if fingerprint.is_empty() {
        return;
    }
    if let (Some(store), Some(profile)) = (app.try_state::<RemoteProfileStore>(), profile) {
        if store.cache(fingerprint, profile).is_ok() {
            let _ = app.emit(PROFILE_UPDATED, fingerprint);
        }
    }
}


fn is_blocked(app: &AppHandle, fingerprint: &str) -> bool {
    app.try_state::<crate::blocked::BlockStore>()
        .is_some_and(|store| store.is_blocked(fingerprint))
}

async fn handle_fast_reconnect_attempt(
    app: AppHandle,
    connection: Connection,
    mut send: SendStream,
    fingerprint: String,
    peer_profile: Option<ProfileWire>,
) -> AppResult<()> {


    if is_blocked(&app, &fingerprint) {
        connection.close(0u32.into(), b"blocked");
        return Ok(());
    }

    let trusted = app
        .try_state::<crate::trusted::store::TrustedStore>()
        .and_then(|store| store.get(&fingerprint).ok().flatten());

    let valid = trusted
        .as_ref()
        .map(|d| !d.is_expired(now_unix_secs()))
        .unwrap_or(false);

    if !valid {
        let _ = write_json_frame(&mut send, &PairingStreamResponse::FastReconnectRejected).await;
        return Ok(());
    }


    cache_peer_profile(&app, &fingerprint, peer_profile);
    write_json_frame(
        &mut send,
        &PairingStreamResponse::FastReconnectAccepted {
            profile: Some(my_profile_wire(&app)),
        },
    )
    .await?;

    let identity = app.state::<DeviceIdentity>();
    let peer_public_key = hex::decode(&fingerprint)
        .map_err(|_| AppError::new("stored fingerprint is not valid hex"))?;

    let secret =
        session::ik_derive_shared_secret_responder(&connection, &identity.private_key, &peer_public_key)
            .await?;

    let display_name = trusted
        .map(|d| d.display_name)
        .unwrap_or_else(|| fingerprint.clone());

    finalize_paired_connection(app, connection, secret, fingerprint, display_name, None).await;
    Ok(())
}


async fn finalize_paired_connection(
    app: AppHandle,
    connection: Connection,
    secret: [u8; 32],
    device_key: String,
    display_name: String,
    expiry: Option<Option<u64>>,
) {
    let state = app.state::<AppState>();
    {
        let mut net = state.net.lock().await;
        net.peers.insert(
            device_key.clone(),
            PeerConnection {
                connection: connection.clone(),
                secret,
                display_name: display_name.clone(),
            },
        );
    }

    spawn_transfer_acceptor(app.clone(), connection.clone(), secret, device_key.clone());
    spawn_uni_acceptor(app.clone(), connection, device_key.clone(), display_name.clone());


    {
        let app = app.clone();
        let device_key = device_key.clone();
        tokio::spawn(async move {
            crate::commands::transfer::resume_jobs_for_device(&app, &device_key).await;
        });
    }

    if let Some(expires_at) = expiry {
        remember_trusted_device(&app, &device_key, &display_name, expires_at).await;
    }

    let _ = app.emit(
        PAIRING_SUCCEEDED,
        PairingSucceededPayload {
            device_key,
            display_name,
        },
    );
}

#[derive(Debug, Clone, Serialize)]
pub struct PairingRequestedPayload {
    pub request_id: String,
    pub requester_name: String,
    pub code: String,


    pub requester_fingerprint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PairingSucceededPayload {
    pub device_key: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PairingFailedPayload {
    pub device_key: String,
    pub reason: String,
}

pub struct PendingInboundRequest {
    pub connection: Connection,
    pub send: SendStream,
    pub session_identity: String,
    pub requester_name: String,
    pub requester_fingerprint: String,
    pub code: String,
    pub requester_msg: Vec<u8>,
    pub expiry_secs: Option<u64>,


    pub requester_profile: Option<ProfileWire>,
}

pub struct PeerConnection {
    pub connection: Connection,
    pub secret: [u8; 32],
    pub display_name: String,
}


pub async fn confirm_inbound_pairing(
    app: AppHandle,
    request_id: String,
    accept: bool,
) -> AppResult<()> {
    let state = app.state::<AppState>();
    let mut pending = {
        let mut net = state.net.lock().await;
        net.inbound_pending
            .remove(&request_id)
            .ok_or_else(|| AppError::new("no such pending pairing request"))?
    };

    if !accept {
        let _ = write_json_frame(&mut pending.send, &PairingStreamResponse::FullRejected).await;
        let _ = app.emit(
            PAIRING_FAILED,
            PairingFailedPayload {
                device_key: pending.requester_name.clone(),
                reason: "rejected".to_string(),
            },
        );
        return Ok(());
    }

    let (exchange, my_msg) = PairingExchange::start(&pending.code, &pending.session_identity);
    let secret = exchange
        .finish(&pending.requester_msg)
        .map_err(|e| AppError::new(format!("pairing key derivation failed: {e}")))?;
    log_key_agreement("responder", &pending.session_identity, &secret);


    let identity = app.state::<DeviceIdentity>();
    let my = my_profile_wire(&app);
    write_json_frame(
        &mut pending.send,
        &PairingStreamResponse::FullAccepted {
            spake_msg: my_msg,
            responder_fingerprint: identity.fingerprint(),
            responder_name: my.display_name.clone(),
            responder_profile: Some(my),
        },
    )
    .await?;

    cache_peer_profile(&app, &pending.requester_fingerprint, pending.requester_profile.take());

    let secret_bytes: [u8; 32] = secret
        .0
        .as_slice()
        .try_into()
        .map_err(|_| AppError::new("unexpected pairing secret length"))?;

    let expires_at = pending.expiry_secs.map(|secs| now_unix_secs() + secs);

    finalize_paired_connection(
        app,
        pending.connection,
        secret_bytes,
        pending.requester_fingerprint,
        pending.requester_name,
        Some(expires_at),
    )
    .await;

    Ok(())
}


async fn remember_trusted_device(
    app: &AppHandle,
    device_key: &str,
    display_name: &str,
    expires_at: Option<u64>,
) {
    if let Some(store) = app.try_state::<crate::trusted::store::TrustedStore>() {


        let existing = store.get(device_key).ok().flatten();
        let added_at = existing
            .as_ref()
            .map(|e| e.added_at.clone())
            .unwrap_or_else(chrono_now);


        let allow_connection_tests =
            existing.as_ref().map(|e| e.allow_connection_tests).unwrap_or(false);
        let nickname = existing.as_ref().and_then(|e| e.nickname.clone());
        let favorite = existing.as_ref().map(|e| e.favorite).unwrap_or(false);
        let auto_accept = existing.as_ref().map(|e| e.auto_accept).unwrap_or(false);

        let _ = store.upsert(TrustedDevice {
            fingerprint: device_key.to_string(),
            display_name: display_name.to_string(),
            added_at,
            expires_at,
            allow_connection_tests,
            nickname,
            favorite,
            auto_accept,
        });
    }
}

fn emit_pairing_failed(app: &AppHandle, device_key: &str, reason: impl Into<String>) {
    let _ = app.emit(
        PAIRING_FAILED,
        PairingFailedPayload {
            device_key: device_key.to_string(),
            reason: reason.into(),
        },
    );
}


pub async fn send_connect_request(
    app: AppHandle,
    endpoint: Endpoint,
    target_addr: std::net::SocketAddr,


    target_fingerprint: Option<String>,
    target_display_name: Option<String>,
    local_display_name: String,
    expiry_secs: Option<u64>,
) -> AppResult<Option<String>> {


    if let Some(fp) = &target_fingerprint {
        let state = app.state::<AppState>();
        let net = state.net.lock().await;
        if net
            .peers
            .get(fp)
            .is_some_and(|peer| peer.connection.close_reason().is_none())
        {
            return Ok(None);
        }
    }

    let mut connection = endpoint
        .connect(target_addr, "zwormhole.local")
        .map_err(|e| AppError::new(format!("failed to start connection: {e}")))?
        .await
        .map_err(|e| AppError::new(format!("failed to connect: {e}")))?;

    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .map_err(|e| AppError::new(format!("failed to open pairing stream: {e}")))?;

    let identity = app.state::<DeviceIdentity>();
    let my_fingerprint = identity.fingerprint();


    let trusted = target_fingerprint.as_deref().and_then(|fp| {
        app.try_state::<crate::trusted::store::TrustedStore>()
            .and_then(|store| store.get(fp).ok().flatten())
    });
    let has_valid_trust = trusted
        .as_ref()
        .map(|d| !d.is_expired(now_unix_secs()))
        .unwrap_or(false);

    if has_valid_trust {
        write_json_frame(
            &mut send,
            &PairingStreamRequest::FastReconnect {
                fingerprint: my_fingerprint.clone(),
                profile: Some(my_profile_wire(&app)),
            },
        )
        .await?;

        match read_json_frame::<PairingStreamResponse>(&mut recv).await {
            Ok(PairingStreamResponse::FastReconnectAccepted { profile }) => {


                let fp = target_fingerprint.clone().unwrap_or_default();
                cache_peer_profile(&app, &fp, profile);
                let peer_public_key = hex::decode(&fp)
                    .map_err(|_| AppError::new("target fingerprint is not valid hex"))?;
                match session::ik_derive_shared_secret_initiator(
                    &connection,
                    &identity.private_key,
                    &peer_public_key,
                )
                .await
                {
                    Ok(secret) => {
                        let name = target_display_name.clone().unwrap_or_else(|| fp.clone());
                        finalize_paired_connection(app, connection, secret, fp, name, None).await;
                        return Ok(None);
                    }
                    Err(e) => {


                        return Err(e);
                    }
                }
            }
            Ok(PairingStreamResponse::FastReconnectRejected) | Err(_) => {


                let fresh = endpoint
                    .connect(target_addr, "zwormhole.local")
                    .map_err(|e| AppError::new(format!("failed to start connection: {e}")))?
                    .await
                    .map_err(|e| AppError::new(format!("failed to connect: {e}")))?;
                let (new_send, new_recv) = fresh
                    .open_bi()
                    .await
                    .map_err(|e| AppError::new(format!("failed to open pairing stream: {e}")))?;
                connection = fresh;
                send = new_send;
                recv = new_recv;
            }
            Ok(_) => return Err(AppError::new("unexpected response to fast-reconnect attempt")),
        }
    }

    let session_identity = uuid::Uuid::new_v4().to_string();
    let code = crate::pairing::confirm_code::generate_code();
    let (exchange, my_msg) = PairingExchange::start(&code, &session_identity);

    write_json_frame(
        &mut send,
        &PairingStreamRequest::Full {
            session_identity: session_identity.clone(),
            requester_name: local_display_name,
            requester_fingerprint: my_fingerprint,
            code: code.clone(),
            spake_msg: my_msg,
            expiry_secs,
            requester_profile: Some(my_profile_wire(&app)),
        },
    )
    .await?;


    let error_key = target_fingerprint
        .clone()
        .unwrap_or_else(|| target_addr.to_string());

    tokio::spawn(async move {
        let response: PairingStreamResponse = match read_json_frame(&mut recv).await {
            Ok(r) => r,
            Err(e) => {
                emit_pairing_failed(&app, &error_key, e.to_string());
                return;
            }
        };

        let (spake_msg, responder_fingerprint, responder_name, responder_profile) = match response {
            PairingStreamResponse::FullAccepted {
                spake_msg,
                responder_fingerprint,
                responder_name,
                responder_profile,
            } => (spake_msg, responder_fingerprint, responder_name, responder_profile),
            PairingStreamResponse::FullRejected => {
                emit_pairing_failed(&app, &error_key, "rejected by peer");
                return;
            }
            _ => {
                emit_pairing_failed(&app, &error_key, "unexpected response");
                return;
            }
        };


        let device_key = match target_fingerprint.clone() {
            Some(fp) => fp,
            None if !responder_fingerprint.is_empty() => responder_fingerprint,
            None => {
                emit_pairing_failed(
                    &app,
                    &error_key,
                    "the other device didn't send its identity (is it an older version?)",
                );
                return;
            }
        };
        let display_name = target_display_name.clone().unwrap_or_else(|| {
            if responder_name.is_empty() {
                device_key.clone()
            } else {
                responder_name
            }
        });


        cache_peer_profile(&app, &device_key, responder_profile);

        let secret = match exchange.finish(&spake_msg) {
            Ok(s) => s,
            Err(e) => {
                emit_pairing_failed(&app, &device_key, e.to_string());
                return;
            }
        };
        log_key_agreement("initiator", &session_identity, &secret);
        let Ok(secret_bytes): Result<[u8; 32], _> = secret.0.as_slice().try_into() else {
            emit_pairing_failed(&app, &device_key, "unexpected pairing secret length");
            return;
        };

        let expires_at = expiry_secs.map(|secs| now_unix_secs() + secs);

        finalize_paired_connection(
            app,
            connection,
            secret_bytes,
            device_key,
            display_name,
            Some(expires_at),
        )
        .await;
    });

    Ok(Some(code))
}


const SPEEDTEST_CHUNK_SIZE: usize = 256 * 1024;
const SPEEDTEST_REPLY_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
enum UniStreamMessage {

    Request { request_id: String, total_bytes: u64 },
    Reply { request_id: String, bytes_received: u64, elapsed_ms: u64 },


    TransferStats {
        transfer_id: String,
        recv_wait_micros: u64,
        disk_write_micros: u64,
        bytes: u64,
    },


    BenchPush { request_id: String, total_bytes: u64 },


    BenchResult { request_id: String, stats: BenchStats },


    BenchPullRequest { request_id: String, total_bytes: u64 },


    ConsentRequest { request_id: String, test_kind: String, total_bytes: u64 },


    ConsentReply { request_id: String, accepted: bool },
}


const CONSENT_TIMEOUT: Duration = Duration::from_secs(45);


const CONSENT_PROMPT_TIMEOUT: Duration = Duration::from_secs(35);


const TEST_ALLOWANCE_WINDOW: Duration = Duration::from_secs(20 * 60);

pub const CONNECTION_TEST_REQUESTED: &str = "zw://conntest/requested";
pub const CONNECTION_TEST_EXPIRED: &str = "zw://conntest/expired";


#[derive(Debug, Clone, Serialize)]
pub struct ConnectionTestRequestPayload {
    pub request_id: String,
    pub device_key: String,
    pub display_name: String,

    pub kind: String,
    pub total_bytes: u64,
}


async fn request_consent(
    app: &AppHandle,
    connection: &Connection,
    kind: &str,
    total_bytes: u64,
) -> AppResult<bool> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let rx = {
        let (tx, rx) = oneshot::channel();
        let state = app.state::<AppState>();
        state
            .net
            .lock()
            .await
            .pending_consent_replies
            .insert(request_id.clone(), tx);
        rx
    };

    let send_result: AppResult<()> = async {
        let mut send = connection
            .open_uni()
            .await
            .map_err(|e| AppError::new(format!("failed to open consent stream: {e}")))?;
        write_uni_frame(
            &mut send,
            &UniStreamMessage::ConsentRequest {
                request_id: request_id.clone(),
                test_kind: kind.to_string(),
                total_bytes,
            },
        )
        .await?;
        send.finish()
            .map_err(|e| AppError::new(format!("failed to finish consent stream: {e}")))?;
        Ok(())
    }
    .await;

    if let Err(e) = send_result {
        app.state::<AppState>()
            .net
            .lock()
            .await
            .pending_consent_replies
            .remove(&request_id);
        return Err(e);
    }

    match tokio::time::timeout(CONSENT_TIMEOUT, rx).await {
        Ok(Ok(accepted)) => Ok(accepted),
        _ => {
            app.state::<AppState>()
                .net
                .lock()
                .await
                .pending_consent_replies
                .remove(&request_id);
            Err(AppError::new(
                "the other device didn't answer the connection-test request in time",
            ))
        }
    }
}


async fn has_test_allowance(app: &AppHandle, device_key: &str) -> bool {
    let state = app.state::<AppState>();
    let mut net = state.net.lock().await;
    match net.test_allowances.get(device_key) {
        Some(expiry) if *expiry > std::time::Instant::now() => true,
        Some(_) => {
            net.test_allowances.remove(device_key);
            false
        }
        None => false,
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchSample {

    pub at_ms: u64,
    pub mbps: f64,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchStats {
    pub bytes: u64,
    pub elapsed_ms: u64,
    pub min_mbps: f64,
    pub max_mbps: f64,
    pub avg_mbps: f64,
    pub samples: Vec<BenchSample>,
}


struct BenchSampler {
    start: std::time::Instant,
    last_at: std::time::Instant,
    last_bytes: u64,
    min_mbps: f64,
    max_mbps: f64,
    samples: Vec<BenchSample>,
}

const BENCH_SAMPLE_INTERVAL: Duration = Duration::from_millis(250);
const BENCH_REPLY_TIMEOUT: Duration = Duration::from_secs(300);

impl BenchSampler {
    fn new() -> Self {
        let now = std::time::Instant::now();
        Self {
            start: now,
            last_at: now,
            last_bytes: 0,
            min_mbps: f64::INFINITY,
            max_mbps: 0.0,
            samples: Vec::new(),
        }
    }

    fn maybe_sample(&mut self, total_bytes: u64) {
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last_at);
        if dt < BENCH_SAMPLE_INTERVAL {
            return;
        }
        let delta = total_bytes.saturating_sub(self.last_bytes);
        let mbps = (delta as f64 * 8.0 / 1_000_000.0) / dt.as_secs_f64();
        self.min_mbps = self.min_mbps.min(mbps);
        self.max_mbps = self.max_mbps.max(mbps);
        self.samples.push(BenchSample {
            at_ms: now.duration_since(self.start).as_millis() as u64,
            mbps,
        });
        self.last_at = now;
        self.last_bytes = total_bytes;
    }

    fn finish(mut self, total_bytes: u64) -> BenchStats {
        let elapsed = self.start.elapsed();
        let secs = elapsed.as_secs_f64();
        let avg = if secs > 0.0 {
            (total_bytes as f64 * 8.0 / 1_000_000.0) / secs
        } else {
            0.0
        };


        if !self.min_mbps.is_finite() {
            self.min_mbps = avg;
        }
        if self.max_mbps == 0.0 {
            self.max_mbps = avg;
        }
        BenchStats {
            bytes: total_bytes,
            elapsed_ms: elapsed.as_millis() as u64,
            min_mbps: self.min_mbps,
            max_mbps: self.max_mbps,
            avg_mbps: avg,
            samples: self.samples,
        }
    }
}


const TRANSFER_STATS_INTERVAL: Duration = Duration::from_millis(300);


const RECEIVE_PROGRESS_INTERVAL: Duration = Duration::from_millis(150);


pub struct SpeedTestReply {
    pub bytes_received: u64,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpeedTestOutcome {
    pub bytes: u64,
    pub send_mbps: f64,
    pub receive_mbps: f64,
}

fn mbps(bytes: u64, elapsed_ms: u64) -> f64 {
    if elapsed_ms == 0 {
        return 0.0;
    }
    (bytes as f64 * 8.0 / 1_000_000.0) / (elapsed_ms as f64 / 1000.0)
}

async fn write_uni_frame(send: &mut SendStream, msg: &UniStreamMessage) -> AppResult<()> {
    let bytes = serde_json::to_vec(msg)
        .map_err(|e| AppError::new(format!("failed to encode speed-test message: {e}")))?;
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    send.write_all(&len.to_be_bytes())
        .await
        .map_err(|e| AppError::new(format!("failed to write speed-test header: {e}")))?;
    send.write_all(&bytes)
        .await
        .map_err(|e| AppError::new(format!("failed to write speed-test header: {e}")))
}

async fn read_uni_frame(recv: &mut RecvStream) -> AppResult<UniStreamMessage> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf)
        .await
        .map_err(|e| AppError::new(format!("failed to read speed-test header: {e}")))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf)
        .await
        .map_err(|e| AppError::new(format!("failed to read speed-test header: {e}")))?;
    serde_json::from_slice(&buf)
        .map_err(|e| AppError::new(format!("failed to decode speed-test header: {e}")))
}


pub async fn run_speed_test(
    app: &AppHandle,
    connection: &Connection,
    total_bytes: u64,
) -> AppResult<SpeedTestOutcome> {
    if !request_consent(app, connection, "speedtest", total_bytes).await? {
        return Err(AppError::new("the other device declined the connection test"));
    }

    let request_id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel();
    {
        let state = app.state::<AppState>();
        let mut net = state.net.lock().await;
        net.pending_speedtests.insert(request_id.clone(), tx);
    }

    let mut send = connection
        .open_uni()
        .await
        .map_err(|e| AppError::new(format!("failed to open speed-test stream: {e}")))?;

    let started = std::time::Instant::now();
    let result: AppResult<()> = async {
        write_uni_frame(
            &mut send,
            &UniStreamMessage::Request {
                request_id: request_id.clone(),
                total_bytes,
            },
        )
        .await?;

        let chunk = vec![0u8; SPEEDTEST_CHUNK_SIZE];
        let mut remaining = total_bytes;
        while remaining > 0 {
            let n = remaining.min(SPEEDTEST_CHUNK_SIZE as u64) as usize;
            send.write_all(&chunk[..n])
                .await
                .map_err(|e| AppError::new(format!("speed-test write failed: {e}")))?;
            remaining -= n as u64;
        }
        send.finish()
            .map_err(|e| AppError::new(format!("failed to finish speed-test stream: {e}")))?;
        Ok(())
    }
    .await;

    if let Err(e) = result {
        let state = app.state::<AppState>();
        let mut net = state.net.lock().await;
        net.pending_speedtests.remove(&request_id);
        return Err(e);
    }
    let send_elapsed_ms = started.elapsed().as_millis() as u64;

    let reply = tokio::time::timeout(SPEEDTEST_REPLY_TIMEOUT, rx)
        .await
        .map_err(|_| AppError::new("timed out waiting for the peer to confirm receipt"))?
        .map_err(|_| AppError::new("connection closed before the peer confirmed receipt"))?;

    Ok(SpeedTestOutcome {
        bytes: total_bytes,
        send_mbps: mbps(total_bytes, send_elapsed_ms),
        receive_mbps: mbps(reply.bytes_received, reply.elapsed_ms),
    })
}


fn spawn_uni_acceptor(
    app: AppHandle,
    connection: Connection,
    device_key: String,
    display_name: String,
) {
    tokio::spawn(async move {
        loop {
            let mut recv = match connection.accept_uni().await {
                Ok(recv) => recv,
                Err(_) => break,
            };

            let app = app.clone();
            let connection = connection.clone();
            let device_key = device_key.clone();
            let display_name = display_name.clone();
            tokio::spawn(async move {
                let header = match read_uni_frame(&mut recv).await {
                    Ok(h) => h,
                    Err(_) => return,
                };

                match header {
                    UniStreamMessage::Request {
                        request_id,
                        total_bytes,
                    } => {


                        if !has_test_allowance(&app, &device_key).await {
                            return;
                        }
                        let started = std::time::Instant::now();
                        let mut discard = vec![0u8; SPEEDTEST_CHUNK_SIZE];
                        let mut remaining = total_bytes;
                        while remaining > 0 {
                            let n = remaining.min(SPEEDTEST_CHUNK_SIZE as u64) as usize;
                            if recv.read_exact(&mut discard[..n]).await.is_err() {
                                return;
                            }
                            remaining -= n as u64;
                        }
                        let elapsed_ms = started.elapsed().as_millis() as u64;

                        let Ok(mut reply_send) = connection.open_uni().await else {
                            return;
                        };
                        let _ = write_uni_frame(
                            &mut reply_send,
                            &UniStreamMessage::Reply {
                                request_id,
                                bytes_received: total_bytes,
                                elapsed_ms,
                            },
                        )
                        .await;
                        let _ = reply_send.finish();
                    }
                    UniStreamMessage::Reply {
                        request_id,
                        bytes_received,
                        elapsed_ms,
                    } => {
                        let state = app.state::<AppState>();
                        let mut net = state.net.lock().await;
                        if let Some(tx) = net.pending_speedtests.remove(&request_id) {
                            let _ = tx.send(SpeedTestReply {
                                bytes_received,
                                elapsed_ms,
                            });
                        }
                    }
                    UniStreamMessage::TransferStats {
                        transfer_id,
                        recv_wait_micros,
                        disk_write_micros,
                        bytes: _,
                    } => {


                        let state = app.state::<AppState>();
                        let updated = {
                            let mut queue = state.transfer_queue.lock().await;
                            queue.set_peer_timing(&transfer_id, recv_wait_micros, disk_write_micros)
                        };
                        if updated {
                            let _ = app.emit(TRANSFER_PROGRESS, &transfer_id);
                        }
                    }
                    UniStreamMessage::BenchPush {
                        request_id,
                        total_bytes,
                    } => {
                        let pull_tx = {
                            let state = app.state::<AppState>();
                            let mut net = state.net.lock().await;
                            net.pending_bench_pulls.remove(&request_id)
                        };
                        match pull_tx {


                            Some(tx) => {
                                if let Ok(stats) =
                                    bench_measure_recv(&mut recv, total_bytes).await
                                {
                                    let _ = tx.send(stats);
                                }
                            }


                            None => {
                                if !has_test_allowance(&app, &device_key).await {
                                    return;
                                }
                                let Ok(stats) = bench_measure_recv(&mut recv, total_bytes).await
                                else {
                                    return;
                                };
                                if let Ok(mut reply) = connection.open_uni().await {
                                    let _ = write_uni_frame(
                                        &mut reply,
                                        &UniStreamMessage::BenchResult { request_id, stats },
                                    )
                                    .await;
                                    let _ = reply.finish();
                                }
                            }
                        }
                    }
                    UniStreamMessage::BenchResult { request_id, stats } => {
                        let state = app.state::<AppState>();
                        let mut net = state.net.lock().await;
                        if let Some(tx) = net.pending_bench_results.remove(&request_id) {
                            let _ = tx.send(stats);
                        }
                    }
                    UniStreamMessage::BenchPullRequest {
                        request_id,
                        total_bytes,
                    } => {


                        if !has_test_allowance(&app, &device_key).await {
                            return;
                        }

                        if let Ok(stats) =
                            bench_push(&connection, request_id.clone(), total_bytes).await
                        {
                            if let Ok(mut reply) = connection.open_uni().await {
                                let _ = write_uni_frame(
                                    &mut reply,
                                    &UniStreamMessage::BenchResult { request_id, stats },
                                )
                                .await;
                                let _ = reply.finish();
                            }
                        }
                    }
                    UniStreamMessage::ConsentRequest {
                        request_id,
                        test_kind,
                        total_bytes,
                    } => {
                        let accepted = handle_consent_request(
                            &app,
                            &device_key,
                            &display_name,
                            &request_id,
                            &test_kind,
                            total_bytes,
                        )
                        .await;
                        if let Ok(mut reply) = connection.open_uni().await {
                            let _ = write_uni_frame(
                                &mut reply,
                                &UniStreamMessage::ConsentReply {
                                    request_id,
                                    accepted,
                                },
                            )
                            .await;
                            let _ = reply.finish();
                        }
                    }
                    UniStreamMessage::ConsentReply {
                        request_id,
                        accepted,
                    } => {
                        let state = app.state::<AppState>();
                        let mut net = state.net.lock().await;
                        if let Some(tx) = net.pending_consent_replies.remove(&request_id) {
                            let _ = tx.send(accepted);
                        }
                    }
                }
            });
        }
    });
}


async fn handle_consent_request(
    app: &AppHandle,
    device_key: &str,
    display_name: &str,
    request_id: &str,
    kind: &str,
    total_bytes: u64,
) -> bool {
    let remembered = app
        .try_state::<crate::trusted::store::TrustedStore>()
        .and_then(|s| s.get(device_key).ok().flatten())
        .map(|d| d.allow_connection_tests)
        .unwrap_or(false);

    let accepted = if remembered {
        true
    } else {
        let rx = {
            let (tx, rx) = oneshot::channel();
            app.state::<AppState>()
                .net
                .lock()
                .await
                .pending_consent_prompts
                .insert(request_id.to_string(), tx);
            rx
        };
        let _ = app.emit(
            CONNECTION_TEST_REQUESTED,
            ConnectionTestRequestPayload {
                request_id: request_id.to_string(),
                device_key: device_key.to_string(),
                display_name: display_name.to_string(),
                kind: kind.to_string(),
                total_bytes,
            },
        );
        let answer = tokio::time::timeout(CONSENT_PROMPT_TIMEOUT, rx).await;
        app.state::<AppState>()
            .net
            .lock()
            .await
            .pending_consent_prompts
            .remove(request_id);
        match answer {
            Ok(Ok((accept, remember))) => {
                if accept && remember {
                    if let Some(store) =
                        app.try_state::<crate::trusted::store::TrustedStore>()
                    {
                        let _ = store.set_allow_connection_tests(device_key, true);
                    }
                }
                accept
            }

            _ => {
                let _ = app.emit(CONNECTION_TEST_EXPIRED, request_id);
                false
            }
        }
    };

    if accepted {
        app.state::<AppState>().net.lock().await.test_allowances.insert(
            device_key.to_string(),
            std::time::Instant::now() + TEST_ALLOWANCE_WINDOW,
        );
    }
    accepted
}


async fn bench_push(
    connection: &Connection,
    request_id: String,
    total_bytes: u64,
) -> AppResult<BenchStats> {
    let mut send = connection
        .open_uni()
        .await
        .map_err(|e| AppError::new(format!("failed to open benchmark stream: {e}")))?;
    write_uni_frame(
        &mut send,
        &UniStreamMessage::BenchPush {
            request_id,
            total_bytes,
        },
    )
    .await?;

    let chunk = vec![0u8; SPEEDTEST_CHUNK_SIZE];
    let mut sampler = BenchSampler::new();
    let mut sent = 0u64;
    let mut remaining = total_bytes;
    while remaining > 0 {
        let n = remaining.min(SPEEDTEST_CHUNK_SIZE as u64) as usize;
        send.write_all(&chunk[..n])
            .await
            .map_err(|e| AppError::new(format!("benchmark write failed: {e}")))?;
        sent += n as u64;
        remaining -= n as u64;
        sampler.maybe_sample(sent);
    }
    send.finish()
        .map_err(|e| AppError::new(format!("failed to finish benchmark stream: {e}")))?;
    Ok(sampler.finish(sent))
}


async fn bench_measure_recv(recv: &mut RecvStream, total_bytes: u64) -> AppResult<BenchStats> {
    let mut discard = vec![0u8; SPEEDTEST_CHUNK_SIZE];
    let mut sampler = BenchSampler::new();
    let mut received = 0u64;
    let mut remaining = total_bytes;
    while remaining > 0 {
        let n = remaining.min(SPEEDTEST_CHUNK_SIZE as u64) as usize;
        recv.read_exact(&mut discard[..n])
            .await
            .map_err(|e| AppError::new(format!("benchmark read failed: {e}")))?;
        received += n as u64;
        remaining -= n as u64;
        sampler.maybe_sample(received);
    }
    Ok(sampler.finish(received))
}


#[derive(Debug, Clone, Serialize)]
pub struct BenchLeg {
    pub sender: BenchStats,
    pub receiver: BenchStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkOutcome {
    pub total_bytes: u64,

    pub upload: BenchLeg,

    pub download: BenchLeg,
}


pub async fn run_benchmark(
    app: &AppHandle,
    connection: &Connection,
    total_bytes: u64,
) -> AppResult<BenchmarkOutcome> {
    if !request_consent(app, connection, "benchmark", total_bytes).await? {
        return Err(AppError::new("the other device declined the benchmark"));
    }


    let up_id = uuid::Uuid::new_v4().to_string();
    let up_rx = {
        let (tx, rx) = oneshot::channel();
        let state = app.state::<AppState>();
        state.net.lock().await.pending_bench_results.insert(up_id.clone(), tx);
        rx
    };
    let upload_sender = match bench_push(connection, up_id.clone(), total_bytes).await {
        Ok(s) => s,
        Err(e) => {
            app.state::<AppState>().net.lock().await.pending_bench_results.remove(&up_id);
            return Err(e);
        }
    };
    let upload_receiver = match tokio::time::timeout(BENCH_REPLY_TIMEOUT, up_rx).await {
        Ok(Ok(stats)) => stats,
        _ => {
            app.state::<AppState>().net.lock().await.pending_bench_results.remove(&up_id);
            return Err(AppError::new("timed out waiting for the peer's upload benchmark stats"));
        }
    };


    let down_id = uuid::Uuid::new_v4().to_string();
    let (down_recv_rx, down_send_rx) = {
        let (pull_tx, pull_rx) = oneshot::channel();
        let (res_tx, res_rx) = oneshot::channel();
        let state = app.state::<AppState>();
        let mut net = state.net.lock().await;
        net.pending_bench_pulls.insert(down_id.clone(), pull_tx);
        net.pending_bench_results.insert(down_id.clone(), res_tx);
        (pull_rx, res_rx)
    };

    let cleanup_download = |app: &AppHandle, id: String| {
        let app = app.clone();
        async move {
            let state = app.state::<AppState>();
            let mut net = state.net.lock().await;
            net.pending_bench_pulls.remove(&id);
            net.pending_bench_results.remove(&id);
        }
    };

    let pull_result: AppResult<()> = async {
        let mut send = connection
            .open_uni()
            .await
            .map_err(|e| AppError::new(format!("failed to open benchmark pull stream: {e}")))?;
        write_uni_frame(
            &mut send,
            &UniStreamMessage::BenchPullRequest {
                request_id: down_id.clone(),
                total_bytes,
            },
        )
        .await?;
        send.finish()
            .map_err(|e| AppError::new(format!("failed to finish benchmark pull request: {e}")))?;
        Ok(())
    }
    .await;
    if let Err(e) = pull_result {
        cleanup_download(app, down_id.clone()).await;
        return Err(e);
    }

    let download_receiver = match tokio::time::timeout(BENCH_REPLY_TIMEOUT, down_recv_rx).await {
        Ok(Ok(stats)) => stats,
        _ => {
            cleanup_download(app, down_id.clone()).await;
            return Err(AppError::new("timed out waiting for the download benchmark data"));
        }
    };
    let download_sender = match tokio::time::timeout(BENCH_REPLY_TIMEOUT, down_send_rx).await {
        Ok(Ok(stats)) => stats,
        _ => {
            cleanup_download(app, down_id.clone()).await;
            return Err(AppError::new("timed out waiting for the peer's download benchmark stats"));
        }
    };

    Ok(BenchmarkOutcome {
        total_bytes,
        upload: BenchLeg {
            sender: upload_sender,
            receiver: upload_receiver,
        },
        download: BenchLeg {
            sender: download_sender,
            receiver: download_receiver,
        },
    })
}
