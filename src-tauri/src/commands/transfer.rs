

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use quinn::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{AppError, AppResult};
use crate::net::{
    self, IncomingDecision, SpeedTestOutcome, TRANSFER_ARCHIVING, TRANSFER_CANCELLED,
    TRANSFER_COMPLETED, TRANSFER_FAILED, TRANSFER_PROGRESS, TRANSFER_REJECTED, TRANSFER_STARTED,
};
use crate::settings::store::AutoExtractMode;
use crate::state::AppState;
use crate::transfer::archive;
use crate::transfer::protocol::{self, SendOutcome};
use crate::transfer::queue::{TransferJob, TransferStatus};
use crate::transfer::queue_store::{PersistedJob, PersistedSource, QueueStore};
use crate::transport::session;


fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}


fn persist_new(
    app: &AppHandle,
    id: &str,
    device_key: &str,
    source: PersistedSource,
    bytes_total: u64,
    status: TransferStatus,
) {
    if let Some(store) = app.try_state::<QueueStore>() {
        let _ = store.upsert(PersistedJob {
            id: id.to_string(),
            device_key: device_key.to_string(),
            source,
            status,
            bytes_total,
            updated_at: now_unix(),
        });
    }
}


fn persist_status(app: &AppHandle, job_id: &str, status: TransferStatus) {
    if let Some(store) = app.try_state::<QueueStore>() {
        let _ = store.set_status(job_id, status, now_unix());
    }
}


const PROGRESS_EMIT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(150);


struct SendSlot(AppHandle);

impl Drop for SendSlot {
    fn drop(&mut self) {
        let state = self.0.state::<AppState>();
        state.active_sends.fetch_sub(1, Ordering::AcqRel);
        state.send_slot_freed.notify_waiters();
    }
}


async fn acquire_send_slot(app: &AppHandle) -> SendSlot {
    let state = app.state::<AppState>();
    loop {
        let max = app
            .try_state::<crate::settings::store::SettingsStore>()
            .map(|s| s.max_concurrent_sends())
            .unwrap_or(0) as usize;
        let cur = state.active_sends.load(Ordering::Acquire);
        if max == 0 || cur < max {
            if state
                .active_sends
                .compare_exchange(cur, cur + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return SendSlot(app.clone());
            }
            continue;
        }
        tokio::select! {
            _ = state.send_slot_freed.notified() => {}
            _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
        }
    }
}

async fn peer_connection(state: &State<'_, AppState>, device_key: &str) -> AppResult<(Connection, [u8; 32])> {
    let net_state = state.net.lock().await;
    let peer = net_state
        .peers
        .get(device_key)
        .ok_or_else(|| AppError::new("not paired with this device — pair first"))?;


    if let Some(reason) = peer.connection.close_reason() {
        return Err(AppError::new(format!(
            "device is no longer connected ({reason}) — reconnect and try again"
        )));
    }
    Ok((peer.connection.clone(), peer.secret))
}


struct TempZipGuard(PathBuf);

impl Drop for TempZipGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}


#[derive(Serialize)]
pub struct SkippedTarget {
    pub device_key: String,
    pub reason: String,
}


#[derive(Serialize)]
pub struct MultiSendResult {
    pub jobs: Vec<TransferJob>,
    pub skipped: Vec<SkippedTarget>,
}


fn label_for_files(paths: &[PathBuf]) -> String {
    let names: Vec<String> = paths
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .collect();
    match names.split_first() {
        Some((first, rest)) if rest.is_empty() => first.clone(),
        Some((first, rest)) => format!("{first} +{}", rest.len()),
        None => "files".to_string(),
    }
}


#[allow(clippy::too_many_arguments)]
fn spawn_send(
    app: AppHandle,
    base_dir: PathBuf,
    paths: Vec<PathBuf>,
    connection: Connection,
    secret: [u8; 32],
    job_id: String,
    bytes_total: u64,
    cleanup: Option<Arc<TempZipGuard>>,
    folder_archive_name: Option<String>,
    device_key: String,
) {


    let history_names: Vec<String> = match &folder_archive_name {
        Some(name) => vec![format!("{name}/ (folder)")],
        None => paths
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .collect(),
    };


    let folder_archive_name_is_folder = folder_archive_name.is_some();
    tokio::spawn(async move {
        let state = app.state::<AppState>();
        let (cancel, pause) = {
            let mut queue = state.transfer_queue.lock().await;
            (queue.cancel_flag(&job_id), queue.pause_flag(&job_id))
        };


        if cancel.load(Ordering::Relaxed) {
            let mut queue = state.transfer_queue.lock().await;
            queue.set_status(&job_id, TransferStatus::Cancelled);
            queue.clear_cancel(&job_id);
            drop(queue);
            persist_status(&app, &job_id, TransferStatus::Cancelled);


            let _ = app.emit(TRANSFER_CANCELLED, &job_id);
            return;
        }


        {
            let mut queue = state.transfer_queue.lock().await;
            queue.set_status(&job_id, TransferStatus::Queued);
        }
        let _slot = acquire_send_slot(&app).await;

        if cancel.load(Ordering::Relaxed) {
            let mut queue = state.transfer_queue.lock().await;
            queue.set_status(&job_id, TransferStatus::Cancelled);
            queue.clear_cancel(&job_id);
            drop(queue);
            persist_status(&app, &job_id, TransferStatus::Cancelled);
            let _ = app.emit(TRANSFER_CANCELLED, &job_id);
            return;
        }

        {
            let mut queue = state.transfer_queue.lock().await;
            queue.set_status(&job_id, TransferStatus::InProgress);
        }
        persist_status(&app, &job_id, TransferStatus::InProgress);
        let _ = app.emit(TRANSFER_STARTED, &job_id);

        let result = async {
            let mut session = session::upgrade_outbound(&connection, &secret).await?;

            let app_for_progress = app.clone();
            let job_id_for_progress = job_id.clone();
            let mut last_emit = Instant::now();
            let on_progress = move |p: protocol::SendProgress| {


                let state = app_for_progress.state::<AppState>();
                if let Ok(mut queue) = state.transfer_queue.try_lock() {
                    queue.set_progress(&job_id_for_progress, p.bytes_sent);
                    queue.set_timing(
                        &job_id_for_progress,
                        p.local_micros,
                        p.encrypt_micros,
                        p.socket_micros,
                    );
                }
                if last_emit.elapsed() >= PROGRESS_EMIT_INTERVAL {
                    last_emit = Instant::now();
                    let _ = app_for_progress.emit(TRANSFER_PROGRESS, &job_id_for_progress);
                }
            };

            let cancel_for_check = cancel.clone();
            let pause_for_check = pause.clone();


            let throttle_app = app.clone();
            let throttle = move || {
                throttle_app
                    .try_state::<crate::settings::store::SettingsStore>()
                    .map(|s| s.max_send_mb_s())
                    .filter(|&m| m > 0)
                    .map(|mb| (mb as u64) * 1_000_000)
            };
            protocol::send_files(
                &mut session,
                &base_dir,
                &paths,
                folder_archive_name,


                Some(job_id.clone()),
                on_progress,
                move || cancel_for_check.load(Ordering::Relaxed),
                move || pause_for_check.load(Ordering::Relaxed),
                throttle,
            )
            .await
        }
        .await;


        let _cleanup = cleanup;

        let mut queue = state.transfer_queue.lock().await;
        queue.clear_cancel(&job_id);
        match result {
            Ok(SendOutcome::Completed) => {
                queue.set_status(&job_id, TransferStatus::Completed);
                queue.set_progress(&job_id, bytes_total);
                drop(queue);
                persist_status(&app, &job_id, TransferStatus::Completed);


                let sent_paths: Vec<String> = if folder_archive_name_is_folder {
                    Vec::new()
                } else {
                    paths.iter().map(|p| p.to_string_lossy().to_string()).collect()
                };
                net::record_transfer_history(
                    &app,
                    &device_key,
                    history_names,
                    "sent",
                    bytes_total,
                    sent_paths,


                    Vec::new(),
                );
                let _ = app.emit(TRANSFER_COMPLETED, &job_id);
            }
            Ok(SendOutcome::Rejected) => {


                queue.set_status(&job_id, TransferStatus::Failed);
                drop(queue);
                persist_status(&app, &job_id, TransferStatus::Failed);
                let _ = app.emit(TRANSFER_REJECTED, &job_id);
            }
            Ok(SendOutcome::Cancelled) => {


                queue.set_status(&job_id, TransferStatus::Cancelled);
                drop(queue);
                persist_status(&app, &job_id, TransferStatus::Cancelled);
                let _ = app.emit(TRANSFER_CANCELLED, &job_id);
            }
            Err(e) => {
                queue.set_status(&job_id, TransferStatus::Failed);
                drop(queue);
                let _ = app.emit(TRANSFER_FAILED, format!("{job_id}: {e}"));


                if connection.close_reason().is_some() {
                    net::disconnect_peer(&app, &device_key).await;
                } else {
                    persist_status(&app, &job_id, TransferStatus::Failed);
                }
            }
        }
    });
}

#[tauri::command]
pub async fn send_files(
    app: AppHandle,
    device_key: String,
    paths: Vec<String>,
) -> AppResult<TransferJob> {
    do_send_files(&app, device_key, paths).await
}


#[tauri::command]
pub async fn send_dropped(
    app: AppHandle,
    device_key: String,
    paths: Vec<String>,
) -> AppResult<()> {
    let mut files = Vec::new();
    for p in paths {
        if std::path::Path::new(&p).is_dir() {
            do_send_folder(&app, device_key.clone(), p, None).await?;
        } else {
            files.push(p);
        }
    }
    if !files.is_empty() {
        do_send_files(&app, device_key, files).await?;
    }
    Ok(())
}


async fn do_send_files(
    app: &AppHandle,
    device_key: String,
    paths: Vec<String>,
) -> AppResult<TransferJob> {
    let state = app.state::<AppState>();
    let (connection, secret) = peer_connection(&state, &device_key).await?;

    let path_strings = paths.clone();
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let base_dir = paths
        .first()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .ok_or_else(|| AppError::new("no files selected"))?;

    let bytes_total: u64 = paths
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();

    let job = {
        let mut queue = state.transfer_queue.lock().await;
        queue.enqueue(device_key.clone(), bytes_total, label_for_files(&paths))
    };
    persist_new(
        app,
        &job.id,
        &device_key,
        PersistedSource::Files { paths: path_strings },
        bytes_total,
        TransferStatus::Queued,
    );

    spawn_send(
        app.clone(),
        base_dir,
        paths,
        connection,
        secret,
        job.id.clone(),
        bytes_total,
        None,
        None,
        device_key,
    );

    Ok(job)
}


#[tauri::command]
pub async fn send_folder(
    app: AppHandle,
    device_key: String,
    folder_path: String,


    selected_files: Option<Vec<String>>,
) -> AppResult<TransferJob> {
    do_send_folder(&app, device_key, folder_path, selected_files).await
}


async fn do_send_folder(
    app: &AppHandle,
    device_key: String,
    folder_path: String,
    selected_files: Option<Vec<String>>,
) -> AppResult<TransferJob> {
    let state = app.state::<AppState>();
    let (connection, secret) = peer_connection(&state, &device_key).await?;

    let folder_path_str = folder_path.clone();
    let folder_path = PathBuf::from(folder_path);
    let folder_name = folder_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| AppError::new("folder has no name"))?;

    let job = {
        let mut queue = state.transfer_queue.lock().await;
        let job = queue.enqueue(device_key.clone(), 0, format!("{folder_name} (folder)"));
        queue.set_status(&job.id, TransferStatus::Archiving);
        job
    };
    persist_new(
        app,
        &job.id,
        &device_key,
        PersistedSource::Folder {
            folder_path: folder_path_str,
            selected_files: selected_files.clone(),
        },
        0,
        TransferStatus::Archiving,
    );
    let _ = app.emit(TRANSFER_ARCHIVING, &job.id);

    let selected_set =
        selected_files.map(|v| v.into_iter().collect::<std::collections::HashSet<String>>());

    let job_id = job.id.clone();
    let app = app.clone();
    tokio::spawn(async move {
        let state = app.state::<AppState>();

        let zip_path = match tokio::task::spawn_blocking({
            let folder_path = folder_path.clone();
            move || match &selected_set {
                Some(set) => archive::zip_folder_selection(&folder_path, set),
                None => archive::zip_folder(&folder_path),
            }
        })
        .await
        {
            Ok(Ok(path)) => path,
            Ok(Err(e)) => {
                let mut queue = state.transfer_queue.lock().await;
                queue.set_status(&job_id, TransferStatus::Failed);
                drop(queue);
                persist_status(&app, &job_id, TransferStatus::Failed);
                let _ = app.emit(TRANSFER_FAILED, format!("{job_id}: {e}"));
                return;
            }
            Err(e) => {
                let mut queue = state.transfer_queue.lock().await;
                queue.set_status(&job_id, TransferStatus::Failed);
                drop(queue);
                persist_status(&app, &job_id, TransferStatus::Failed);
                let _ = app.emit(TRANSFER_FAILED, format!("{job_id}: archiving task failed: {e}"));
                return;
            }
        };

        let bytes_total = std::fs::metadata(&zip_path).map(|m| m.len()).unwrap_or(0);
        let base_dir = match zip_path.parent().map(|p| p.to_path_buf()) {
            Some(p) => p,
            None => {
                let mut queue = state.transfer_queue.lock().await;
                queue.set_status(&job_id, TransferStatus::Failed);
                drop(queue);
                persist_status(&app, &job_id, TransferStatus::Failed);
                let _ = app.emit(TRANSFER_FAILED, format!("{job_id}: archive has no parent directory"));
                return;
            }
        };
        {
            let mut queue = state.transfer_queue.lock().await;
            queue.set_total(&job_id, bytes_total);
        }

        spawn_send(
            app,
            base_dir,
            vec![zip_path.clone()],
            connection,
            secret,
            job_id,
            bytes_total,
            Some(Arc::new(TempZipGuard(zip_path))),
            Some(folder_name),
            device_key,
        );
    });

    Ok(job)
}


#[tauri::command]
pub async fn send_clipboard_image(
    app: AppHandle,
    state: State<'_, AppState>,
    device_key: String,
) -> AppResult<TransferJob> {
    let (connection, secret) = peer_connection(&state, &device_key).await?;
    let png_path = crate::platform::clipboard_image(&app).await?;

    let base_dir = png_path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| AppError::new("temp image has no parent directory"))?;
    let bytes_total = std::fs::metadata(&png_path).map(|m| m.len()).unwrap_or(0);

    let job = {
        let mut queue = state.transfer_queue.lock().await;
        queue.enqueue(device_key.clone(), bytes_total, "Clipboard image".to_string())
    };
    spawn_send(
        app,
        base_dir,
        vec![png_path.clone()],
        connection,
        secret,
        job.id.clone(),
        bytes_total,
        Some(Arc::new(TempZipGuard(png_path))),
        None,
        device_key,
    );
    Ok(job)
}


pub async fn resume_jobs_for_device(app: &AppHandle, device_key: &str) {
    let store = match app.try_state::<QueueStore>() {
        Some(s) => s,
        None => return,
    };
    let pending: Vec<PersistedJob> = store
        .list()
        .unwrap_or_default()
        .into_iter()
        .filter(|j| j.device_key == device_key)
        .collect();
    if pending.is_empty() {
        return;
    }


    {
        let state = app.state::<AppState>();
        let queue = state.transfer_queue.lock().await;
        let busy = queue
            .list()
            .iter()
            .any(|j| j.device_fullname == device_key && !j.status.is_terminal());
        if busy {
            return;
        }
    }

    for pj in pending {


        match pj.source {
            PersistedSource::Files { paths } => {
                let existing: Vec<String> = paths
                    .into_iter()
                    .filter(|p| std::path::Path::new(p).exists())
                    .collect();
                if existing.is_empty() {
                    let _ = store.remove(&pj.id);
                    let _ = app.emit(
                        TRANSFER_FAILED,
                        format!("{}: source files no longer exist — can't resume", pj.id),
                    );
                    continue;
                }
                if do_send_files(app, device_key.to_string(), existing).await.is_ok() {
                    let _ = store.remove(&pj.id);
                }
            }
            PersistedSource::Folder {
                folder_path,
                selected_files,
            } => {
                if !std::path::Path::new(&folder_path).exists() {
                    let _ = store.remove(&pj.id);
                    let _ = app.emit(
                        TRANSFER_FAILED,
                        format!("{}: source folder no longer exists — can't resume", pj.id),
                    );
                    continue;
                }
                if do_send_folder(app, device_key.to_string(), folder_path, selected_files)
                    .await
                    .is_ok()
                {
                    let _ = store.remove(&pj.id);
                }
            }
        }
    }
}


#[tauri::command]
pub async fn list_resumable_transfers(app: AppHandle) -> AppResult<Vec<PersistedJob>> {
    match app.try_state::<QueueStore>() {
        Some(store) => store.list(),
        None => Ok(Vec::new()),
    }
}


#[tauri::command]
pub async fn resume_device_transfers(app: AppHandle, device_key: String) -> AppResult<()> {
    resume_jobs_for_device(&app, &device_key).await;
    Ok(())
}


#[tauri::command]
pub async fn dismiss_resumable(app: AppHandle, job_id: String) -> AppResult<()> {
    if let Some(store) = app.try_state::<QueueStore>() {
        store.remove(&job_id)?;
    }
    Ok(())
}


#[tauri::command]
pub async fn send_files_multi(
    app: AppHandle,
    state: State<'_, AppState>,
    device_keys: Vec<String>,
    paths: Vec<String>,
) -> AppResult<MultiSendResult> {
    let path_strings = paths.clone();
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let base_dir = paths
        .first()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .ok_or_else(|| AppError::new("no files selected"))?;
    let bytes_total: u64 = paths
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();

    let mut jobs = Vec::new();
    let mut skipped = Vec::new();
    for device_key in device_keys {
        match peer_connection(&state, &device_key).await {
            Ok((connection, secret)) => {
                let job = {
                    let mut queue = state.transfer_queue.lock().await;
                    queue.enqueue(device_key.clone(), bytes_total, label_for_files(&paths))
                };
                persist_new(
                    &app,
                    &job.id,
                    &device_key,
                    PersistedSource::Files {
                        paths: path_strings.clone(),
                    },
                    bytes_total,
                    TransferStatus::Queued,
                );
                spawn_send(
                    app.clone(),
                    base_dir.clone(),
                    paths.clone(),
                    connection,
                    secret,
                    job.id.clone(),
                    bytes_total,
                    None,
                    None,
                    device_key,
                );
                jobs.push(job);
            }
            Err(e) => skipped.push(SkippedTarget {
                device_key,
                reason: e.to_string(),
            }),
        }
    }

    if jobs.is_empty() {
        return Err(AppError::new(
            "none of the selected devices were reachable — reconnect and try again",
        ));
    }
    Ok(MultiSendResult { jobs, skipped })
}


fn fail_multi_targets(
    app: &AppHandle,
    queue: &mut crate::transfer::queue::TransferQueue,
    targets: &[(String, Connection, [u8; 32], TransferJob)],
    msg: &str,
) {
    for (_, _, _, job) in targets {
        queue.set_status(&job.id, TransferStatus::Failed);
        persist_status(app, &job.id, TransferStatus::Failed);
        let _ = app.emit(TRANSFER_FAILED, format!("{}: {msg}", job.id));
    }
}


#[tauri::command]
pub async fn send_folder_multi(
    app: AppHandle,
    state: State<'_, AppState>,
    device_keys: Vec<String>,
    folder_path: String,
    selected_files: Option<Vec<String>>,
) -> AppResult<MultiSendResult> {
    let folder_path_str = folder_path.clone();
    let folder_path = PathBuf::from(folder_path);
    let folder_name = folder_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| AppError::new("folder has no name"))?;


    let mut targets: Vec<(String, Connection, [u8; 32], TransferJob)> = Vec::new();
    let mut skipped = Vec::new();
    for device_key in device_keys {
        match peer_connection(&state, &device_key).await {
            Ok((connection, secret)) => {
                let job = {
                    let mut queue = state.transfer_queue.lock().await;
                    let job = queue.enqueue(device_key.clone(), 0, format!("{folder_name} (folder)"));
                    queue.set_status(&job.id, TransferStatus::Archiving);
                    job
                };
                persist_new(
                    &app,
                    &job.id,
                    &device_key,
                    PersistedSource::Folder {
                        folder_path: folder_path_str.clone(),
                        selected_files: selected_files.clone(),
                    },
                    0,
                    TransferStatus::Archiving,
                );
                let _ = app.emit(TRANSFER_ARCHIVING, &job.id);
                targets.push((device_key, connection, secret, job));
            }
            Err(e) => skipped.push(SkippedTarget {
                device_key,
                reason: e.to_string(),
            }),
        }
    }
    if targets.is_empty() {
        return Err(AppError::new(
            "none of the selected devices were reachable — reconnect and try again",
        ));
    }

    let jobs: Vec<TransferJob> = targets.iter().map(|(_, _, _, j)| j.clone()).collect();
    let selected_set =
        selected_files.map(|v| v.into_iter().collect::<std::collections::HashSet<String>>());

    tokio::spawn(async move {
        let state = app.state::<AppState>();

        let zip_path = match tokio::task::spawn_blocking({
            let folder_path = folder_path.clone();
            move || match &selected_set {
                Some(set) => archive::zip_folder_selection(&folder_path, set),
                None => archive::zip_folder(&folder_path),
            }
        })
        .await
        {
            Ok(Ok(path)) => path,
            Ok(Err(e)) => {
                let mut queue = state.transfer_queue.lock().await;
                fail_multi_targets(&app, &mut queue, &targets, &e.to_string());
                return;
            }
            Err(e) => {
                let mut queue = state.transfer_queue.lock().await;
                fail_multi_targets(&app, &mut queue, &targets, &format!("archiving task failed: {e}"));
                return;
            }
        };

        let bytes_total = std::fs::metadata(&zip_path).map(|m| m.len()).unwrap_or(0);
        let base_dir = match zip_path.parent().map(|p| p.to_path_buf()) {
            Some(p) => p,
            None => {
                let mut queue = state.transfer_queue.lock().await;
                fail_multi_targets(&app, &mut queue, &targets, "archive has no parent directory");
                return;
            }
        };
        {
            let mut queue = state.transfer_queue.lock().await;
            for (_, _, _, job) in &targets {
                queue.set_total(&job.id, bytes_total);
            }
        }


        let guard = Arc::new(TempZipGuard(zip_path.clone()));
        for (device_key, connection, secret, job) in targets {
            spawn_send(
                app.clone(),
                base_dir.clone(),
                vec![zip_path.clone()],
                connection,
                secret,
                job.id.clone(),
                bytes_total,
                Some(guard.clone()),
                Some(folder_name.clone()),
                device_key,
            );
        }
    });

    Ok(MultiSendResult { jobs, skipped })
}


#[tauri::command]
pub async fn respond_to_incoming_transfer(
    state: State<'_, AppState>,
    request_id: String,
    accept: bool,
    save_dir: Option<String>,
    extract_mode: Option<AutoExtractMode>,
) -> AppResult<()> {
    let mut net = state.net.lock().await;
    if let Some(tx) = net.pending_incoming.remove(&request_id) {
        let _ = tx.send(IncomingDecision {
            accept,
            save_dir: save_dir.map(PathBuf::from),
            extract_mode,
        });
    }
    Ok(())
}


#[tauri::command]
pub async fn run_speed_test(
    app: AppHandle,
    state: State<'_, AppState>,
    device_key: String,
    size_mb: u64,
) -> AppResult<SpeedTestOutcome> {
    let (connection, _secret) = peer_connection(&state, &device_key).await?;
    net::run_speed_test(&app, &connection, size_mb * 1024 * 1024).await
}


#[tauri::command]
pub fn read_folder_tree(path: String) -> AppResult<crate::transfer::tree::FolderNode> {
    crate::transfer::tree::read_tree(std::path::Path::new(&path))
}


#[tauri::command]
pub async fn send_text(app: AppHandle, device_key: String, text: String) -> AppResult<()> {
    net::send_text_message(&app, &device_key, text).await
}


#[tauri::command]
pub async fn run_benchmark(
    app: AppHandle,
    state: State<'_, AppState>,
    device_key: String,
    size_mb: u64,
) -> AppResult<net::BenchmarkOutcome> {
    let (connection, _secret) = peer_connection(&state, &device_key).await?;
    net::run_benchmark(&app, &connection, size_mb * 1024 * 1024).await
}


#[tauri::command]
pub async fn respond_to_connection_test(
    state: State<'_, AppState>,
    request_id: String,
    accept: bool,
    remember: bool,
) -> AppResult<()> {
    let mut net = state.net.lock().await;
    if let Some(tx) = net.pending_consent_prompts.remove(&request_id) {
        let _ = tx.send((accept, remember));
    }
    Ok(())
}


#[tauri::command]
pub async fn export_benchmark(path: String, contents: String) -> AppResult<()> {
    std::fs::write(&path, &contents)
        .map_err(|e| AppError::new(format!("failed to write {path}: {e}")))
}


#[tauri::command]
pub fn record_benchmark(
    app: AppHandle,
    fingerprint: String,
    display_name: String,
    up_avg_mbps: f64,
    down_avg_mbps: f64,
) -> AppResult<()> {
    if let Some(store) = app.try_state::<crate::bench_history::BenchHistoryStore>() {
        store.record(crate::bench_history::BenchHistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            fingerprint,
            display_name,
            at_unix: now_unix(),
            up_avg_mbps,
            down_avg_mbps,
        })?;
    }
    Ok(())
}

#[tauri::command]
pub fn list_benchmark_history(
    app: AppHandle,
) -> AppResult<Vec<crate::bench_history::BenchHistoryEntry>> {
    match app.try_state::<crate::bench_history::BenchHistoryStore>() {
        Some(store) => store.list(),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
pub fn clear_benchmark_history(app: AppHandle) -> AppResult<()> {
    if let Some(store) = app.try_state::<crate::bench_history::BenchHistoryStore>() {
        store.clear()?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_transfers(state: State<'_, AppState>) -> AppResult<Vec<TransferJob>> {
    let queue = state.transfer_queue.lock().await;
    Ok(queue.list())
}

#[tauri::command]
pub fn list_transfer_history(
    history: State<'_, crate::history::store::HistoryStore>,
) -> AppResult<Vec<crate::history::store::HistoryEntry>> {
    history.list()
}

#[tauri::command]
pub fn clear_transfer_history(
    history: State<'_, crate::history::store::HistoryStore>,
) -> AppResult<()> {
    history.clear()
}


#[tauri::command]
pub async fn cancel_transfer(state: State<'_, AppState>, job_id: String) -> AppResult<()> {
    let mut queue = state.transfer_queue.lock().await;
    match queue.get(&job_id).map(|j| j.status) {
        None => return Err(AppError::new("no such transfer job")),
        Some(TransferStatus::Queued)
        | Some(TransferStatus::Archiving)
        | Some(TransferStatus::InProgress)
        | Some(TransferStatus::Paused) => {}

        Some(_) => return Ok(()),
    }


    queue.cancel_flag(&job_id).store(true, Ordering::Relaxed);


    queue.pause_flag(&job_id).store(false, Ordering::Relaxed);
    queue.set_status(&job_id, TransferStatus::Cancelled);
    Ok(())
}


#[tauri::command]
pub async fn pause_transfer(app: AppHandle, state: State<'_, AppState>, job_id: String) -> AppResult<()> {
    let mut queue = state.transfer_queue.lock().await;
    match queue.get(&job_id).map(|j| j.status) {
        Some(TransferStatus::InProgress) => {}
        _ => return Ok(()),
    }
    queue.pause_flag(&job_id).store(true, Ordering::Relaxed);
    queue.set_status(&job_id, TransferStatus::Paused);
    drop(queue);
    persist_status(&app, &job_id, TransferStatus::Paused);
    let _ = app.emit(TRANSFER_PROGRESS, &job_id);
    Ok(())
}


#[tauri::command]
pub async fn resume_transfer(app: AppHandle, state: State<'_, AppState>, job_id: String) -> AppResult<()> {
    let mut queue = state.transfer_queue.lock().await;
    match queue.get(&job_id).map(|j| j.status) {
        Some(TransferStatus::Paused) => {}
        _ => return Ok(()),
    }
    queue.pause_flag(&job_id).store(false, Ordering::Relaxed);
    queue.set_status(&job_id, TransferStatus::InProgress);
    drop(queue);
    persist_status(&app, &job_id, TransferStatus::InProgress);
    let _ = app.emit(TRANSFER_PROGRESS, &job_id);
    Ok(())
}
