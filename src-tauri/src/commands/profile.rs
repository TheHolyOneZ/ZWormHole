

use base64::Engine;
use tauri::{AppHandle, Manager, State};

use crate::commands::discovery::local_display_name;
use crate::error::{AppError, AppResult};
use crate::profile::{MyProfileStore, RemoteProfileStore};

fn to_data_url(png: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
    format!("data:image/png;base64,{b64}")
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MyProfilePayload {

    pub display_name: Option<String>,

    pub effective_name: String,

    pub avatar_data_url: Option<String>,
}

#[tauri::command]
pub fn get_my_profile(store: State<'_, MyProfileStore>) -> AppResult<MyProfilePayload> {
    let display_name = store.display_name_override();
    let effective_name = display_name.clone().unwrap_or_else(local_display_name);
    Ok(MyProfilePayload {
        display_name,
        effective_name,
        avatar_data_url: store.avatar_bytes().map(|b| to_data_url(&b)),
    })
}

#[tauri::command]
pub fn set_my_display_name(
    store: State<'_, MyProfileStore>,
    name: Option<String>,
) -> AppResult<()> {
    store.set_display_name(name)
}


#[tauri::command]
pub fn set_my_avatar(store: State<'_, MyProfileStore>, path: String) -> AppResult<()> {
    let bytes =
        std::fs::read(&path).map_err(|e| AppError::new(format!("couldn't read that file: {e}")))?;
    store.set_avatar(&bytes)
}

#[tauri::command]
pub fn clear_my_avatar(store: State<'_, MyProfileStore>) -> AppResult<()> {
    store.clear_avatar()
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceProfilePayload {
    pub fingerprint: String,
    pub display_name: String,
    pub avatar_data_url: Option<String>,
}


#[tauri::command]
pub fn list_profiles(store: State<'_, RemoteProfileStore>) -> AppResult<Vec<DeviceProfilePayload>> {
    Ok(store
        .list()
        .into_iter()
        .map(|(fingerprint, p)| DeviceProfilePayload {
            fingerprint,
            display_name: p.display_name,
            avatar_data_url: p.avatar_png.map(|b| to_data_url(&b)),
        })
        .collect())
}


#[tauri::command]
pub async fn refresh_advertisement(app: AppHandle) -> AppResult<()> {
    let was_visible = {
        let state = app.state::<crate::state::AppState>();
        let discovery = state.discovery.lock().await;
        discovery.own_fullname.is_some()
    };
    if was_visible {
        let _ = crate::commands::discovery::stop_visible(app.clone(), app.state()).await;
        let _ = crate::commands::discovery::become_visible(app.clone(), app.state()).await;
    }
    Ok(())
}
