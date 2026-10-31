use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::error::AppResult;
use crate::settings::store::{AutoExtractMode, SettingsStore};
use crate::state::AppState;

#[tauri::command]
pub fn get_background_listener_enabled(state: State<'_, AppState>) -> AppResult<bool> {
    Ok(state.background_listener_enabled.load(Ordering::Relaxed))
}

#[tauri::command]
pub fn set_background_listener_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    store: State<'_, SettingsStore>,
    enabled: bool,
) -> AppResult<()> {
    store.set_background_listener(enabled)?;
    let previous = state
        .background_listener_enabled
        .swap(enabled, Ordering::Relaxed);
    if previous != enabled {
        crate::platform::set_background_service(&app, enabled);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferSettingsPayload {
    pub auto_accept: bool,
    pub save_dir: Option<String>,


    pub default_save_dir: Option<String>,
    pub auto_extract: AutoExtractMode,

    pub max_send_mb_s: u32,

    pub max_concurrent_sends: u32,


    pub quic_port: u16,
}

#[tauri::command]
pub fn get_transfer_settings(
    app: AppHandle,
    store: State<'_, SettingsStore>,
) -> AppResult<TransferSettingsPayload> {
    let settings = store.get();
    let default_save_dir = crate::platform::default_save_dir(&app)
        .ok()
        .map(|p| p.to_string_lossy().to_string());
    Ok(TransferSettingsPayload {
        auto_accept: settings.auto_accept,
        save_dir: settings.save_dir.map(|p| p.to_string_lossy().to_string()),
        default_save_dir,
        auto_extract: settings.auto_extract,
        max_send_mb_s: settings.max_send_mb_s,
        max_concurrent_sends: settings.max_concurrent_sends,
        quic_port: settings.quic_port,
    })
}

#[tauri::command]
pub fn set_auto_accept(store: State<'_, SettingsStore>, enabled: bool) -> AppResult<()> {
    store.set_auto_accept(enabled)
}

#[tauri::command]
pub fn set_save_dir(
    app: AppHandle,
    store: State<'_, SettingsStore>,
    dir: Option<String>,
) -> AppResult<()> {
    match dir {
        Some(dir) => {
            let dir = std::path::PathBuf::from(dir);
            crate::platform::ensure_writable_dir(&dir)?;
            store.set_save_dir(Some(dir))
        }
        None => {
            crate::platform::forget_save_dir(&app);
            store.set_save_dir(None)
        }
    }
}

#[tauri::command]
pub fn set_auto_extract(store: State<'_, SettingsStore>, mode: AutoExtractMode) -> AppResult<()> {
    store.set_auto_extract(mode)
}

#[tauri::command]
pub fn set_max_send_mb_s(store: State<'_, SettingsStore>, value: u32) -> AppResult<()> {
    store.set_max_send_mb_s(value)
}

#[tauri::command]
pub fn set_max_concurrent_sends(store: State<'_, SettingsStore>, value: u32) -> AppResult<()> {
    store.set_max_concurrent_sends(value)
}


#[tauri::command]
pub async fn set_quic_port(
    app: AppHandle,
    store: State<'_, SettingsStore>,
    port: u16,
) -> AppResult<u16> {
    let previous = store.quic_port();


    let was_visible = {
        let state = app.state::<AppState>();
        let discovery = state.discovery.lock().await;
        discovery.own_fullname.is_some()
    };
    if was_visible {
        let _ = crate::commands::discovery::stop_visible(app.clone(), app.state::<AppState>()).await;
    }

    store.set_quic_port(port)?;
    crate::net::reset_endpoint(&app).await;

    let bound = match crate::net::ensure_endpoint(&app).await {
        Ok(ep) => ep,
        Err(e) => {


            store.set_quic_port(previous)?;
            crate::net::reset_endpoint(&app).await;
            let _ = crate::net::ensure_endpoint(&app).await;
            if was_visible {
                let _ = crate::commands::discovery::become_visible(
                    app.clone(),
                    app.state::<AppState>(),
                )
                .await;
            }
            return Err(e);
        }
    };

    let actual = crate::transport::endpoint::local_port(&bound)?;
    if was_visible {
        let _ =
            crate::commands::discovery::become_visible(app.clone(), app.state::<AppState>()).await;
    }
    Ok(actual)
}
