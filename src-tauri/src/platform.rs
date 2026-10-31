use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

static DEVICE_NAME: OnceLock<String> = OnceLock::new();
static PUBLIC_SAVE_DIR: OnceLock<PathBuf> = OnceLock::new();

const IMPORT_RETENTION_SECS: u64 = 3 * 24 * 60 * 60;

#[derive(Debug, Clone, Serialize)]
pub struct PlatformInfo {
    pub os: &'static str,
    pub mobile: bool,
}

pub fn info() -> PlatformInfo {
    PlatformInfo {
        os: os_badge(),
        mobile: cfg!(mobile),
    }
}

pub fn os_badge() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else {
        "linux"
    }
}

pub fn device_name() -> String {
    if let Some(name) = DEVICE_NAME.get() {
        return name.clone();
    }
    let host = gethostname::gethostname()
        .into_string()
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "localhost");
    host.unwrap_or_else(|| match os_badge() {
        "android" => "Android".to_string(),
        "ios" => "iPhone".to_string(),
        _ => "ZWormHole Device".to_string(),
    })
}

pub fn default_save_dir(app: &AppHandle) -> AppResult<PathBuf> {
    if let Some(dir) = PUBLIC_SAVE_DIR.get() {
        return Ok(dir.clone());
    }
    app.path()
        .download_dir()
        .map_err(|e| AppError::new(format!("failed to resolve downloads dir: {e}")))
}

fn imports_root(app: &AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map(|d| d.join("imports"))
        .map_err(|e| AppError::new(format!("failed to resolve app data dir: {e}")))
}

pub fn new_import_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = imports_root(app)?.join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::new(format!("failed to prepare import folder: {e}")))?;
    Ok(dir)
}

fn prune_old_imports(app: &AppHandle) {
    let Ok(root) = imports_root(app) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    let now = std::time::SystemTime::now();
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|age| age.as_secs() > IMPORT_RETENTION_SECS);
        if old {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

pub fn ensure_writable_dir(dir: &Path) -> AppResult<()> {
    std::fs::create_dir_all(dir)
        .map_err(|e| AppError::new(format!("can't use {}: {e}", dir.display())))?;
    let probe = dir.join(format!(".zwormhole-{}", uuid::Uuid::new_v4()));
    std::fs::write(&probe, b"")
        .map_err(|e| AppError::new(format!("ZWormHole can't write to {}: {e}", dir.display())))?;
    let _ = std::fs::remove_file(probe);
    Ok(())
}

pub fn init(app: &AppHandle) {
    prune_old_imports(app);
    #[cfg(mobile)]
    native::init(app);
}

#[cfg(desktop)]
pub async fn open_url(_app: &AppHandle, url: &str) -> AppResult<()> {
    desktop::open_url(url)
}

#[cfg(desktop)]
pub async fn open_path(_app: &AppHandle, path: &str) -> AppResult<()> {
    desktop::open_path(path)
}

#[cfg(desktop)]
pub async fn reveal_path(_app: &AppHandle, path: &str) -> AppResult<()> {
    desktop::reveal_path(path)
}

#[cfg(desktop)]
pub async fn clipboard_image(_app: &AppHandle) -> AppResult<PathBuf> {
    tokio::task::spawn_blocking(desktop::clipboard_image_to_png)
        .await
        .map_err(|e| AppError::new(format!("clipboard task failed: {e}")))?
}

#[cfg(desktop)]
pub fn files_saved(_app: &AppHandle, _paths: &[PathBuf]) {}

#[cfg(desktop)]
pub fn set_background_service(_app: &AppHandle, _enabled: bool) {}

#[cfg(desktop)]
pub async fn pick_files(_app: &AppHandle, _images: bool) -> AppResult<Vec<String>> {
    Err(AppError::new("native pickers are only used on mobile"))
}

#[cfg(desktop)]
pub async fn pick_folder(_app: &AppHandle) -> AppResult<Option<String>> {
    Err(AppError::new("native pickers are only used on mobile"))
}

#[cfg(desktop)]
pub async fn pick_save_dir(_app: &AppHandle, _persist: bool) -> AppResult<Option<String>> {
    Err(AppError::new("native pickers are only used on mobile"))
}

#[cfg(desktop)]
pub fn forget_save_dir(_app: &AppHandle) {}

#[cfg(desktop)]
pub async fn export_file(_app: &AppHandle, _source: PathBuf, _mime: &str) -> AppResult<bool> {
    Err(AppError::new("native export is only used on mobile"))
}

#[cfg(mobile)]
pub use native::{
    clipboard_image, export_file, files_saved, forget_save_dir, open_path, open_url, pick_files,
    pick_folder, pick_save_dir, reveal_path, set_background_service,
};

#[cfg(target_os = "ios")]
pub use native::bonjour;

#[cfg(desktop)]
mod desktop {
    use std::path::PathBuf;

    use crate::error::{AppError, AppResult};

    pub fn clipboard_image_to_png() -> AppResult<PathBuf> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| AppError::new(format!("clipboard unavailable: {e}")))?;
        let image = clipboard
            .get_image()
            .map_err(|e| AppError::new(format!("no image in the clipboard: {e}")))?;
        let path =
            std::env::temp_dir().join(format!("zwormhole-clip-{}.png", uuid::Uuid::new_v4()));
        let file = std::fs::File::create(&path)
            .map_err(|e| AppError::new(format!("failed to create temp image: {e}")))?;
        let mut encoder = png::Encoder::new(
            std::io::BufWriter::new(file),
            image.width as u32,
            image.height as u32,
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| AppError::new(format!("failed to write png header: {e}")))?;
        writer
            .write_image_data(&image.bytes)
            .map_err(|e| AppError::new(format!("failed to encode png: {e}")))?;
        Ok(path)
    }

    #[cfg(target_os = "linux")]
    pub fn open_url(url: &str) -> AppResult<()> {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::new(format!("failed to launch xdg-open: {e}")))
    }

    #[cfg(target_os = "linux")]
    pub fn open_path(path: &str) -> AppResult<()> {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::new(format!("failed to open file: {e}")))
    }

    #[cfg(target_os = "linux")]
    pub fn reveal_path(path: &str) -> AppResult<()> {
        let dir = std::path::Path::new(path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        std::process::Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::new(format!("failed to open folder: {e}")))
    }

    #[cfg(target_os = "macos")]
    pub fn open_url(url: &str) -> AppResult<()> {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::new(format!("failed to launch open: {e}")))
    }

    #[cfg(target_os = "macos")]
    pub fn open_path(path: &str) -> AppResult<()> {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::new(format!("failed to open file: {e}")))
    }

    #[cfg(target_os = "macos")]
    pub fn reveal_path(path: &str) -> AppResult<()> {
        std::process::Command::new("open")
            .args(["-R", path])
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::new(format!("failed to reveal file: {e}")))
    }

    #[cfg(target_os = "windows")]
    pub fn open_url(url: &str) -> AppResult<()> {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::new(format!("failed to launch start: {e}")))
    }

    #[cfg(target_os = "windows")]
    pub fn open_path(path: &str) -> AppResult<()> {
        use std::os::windows::process::CommandExt;

        std::process::Command::new("cmd")
            .raw_arg(format!("/C start \"\" \"{path}\""))
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::new(format!("failed to open file: {e}")))
    }

    #[cfg(target_os = "windows")]
    pub fn reveal_path(path: &str) -> AppResult<()> {
        use std::os::windows::process::CommandExt;

        std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{path}\""))
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::new(format!("failed to reveal file: {e}")))
    }
}

#[cfg(mobile)]
mod native {
    use std::path::PathBuf;

    use serde::de::DeserializeOwned;
    use serde::{Deserialize, Serialize};
    use serde_json::{json, Value};
    use tauri::AppHandle;
    use tauri_plugin_zwormhole_mobile::MobileExt;

    use super::{DEVICE_NAME, PUBLIC_SAVE_DIR};
    use crate::error::{AppError, AppResult};

    #[derive(Debug, Default, Deserialize)]
    #[serde(default, rename_all = "camelCase")]
    struct PathsResponse {
        paths: Vec<String>,
        cancelled: bool,
    }

    #[derive(Debug, Default, Deserialize)]
    #[serde(default, rename_all = "camelCase")]
    struct PathResponse {
        path: Option<String>,
        cancelled: bool,
    }

    #[derive(Debug, Default, Deserialize)]
    #[serde(default, rename_all = "camelCase")]
    struct SavedResponse {
        saved: bool,
    }

    #[derive(Debug, Default, Deserialize)]
    #[serde(default, rename_all = "camelCase")]
    struct DeviceInfoResponse {
        name: String,
        download_dir: String,
    }

    fn call<T: DeserializeOwned>(
        app: &AppHandle,
        command: &str,
        payload: impl Serialize,
    ) -> AppResult<T> {
        app.native().call(command, payload).map_err(AppError::new)
    }

    async fn call_blocking<T: DeserializeOwned + Send + 'static>(
        app: &AppHandle,
        command: &'static str,
        payload: Value,
    ) -> AppResult<T> {
        let app = app.clone();
        tokio::task::spawn_blocking(move || call(&app, command, payload))
            .await
            .map_err(|e| AppError::new(format!("native call failed: {e}")))?
    }

    fn detached(app: &AppHandle, work: impl FnOnce(&AppHandle) + Send + 'static) {
        let app = app.clone();
        std::thread::spawn(move || work(&app));
    }

    pub fn init(app: &AppHandle) {
        detached(app, |app| {
            if let Ok(info) = call::<DeviceInfoResponse>(app, "deviceInfo", json!({})) {
                let name = info.name.trim().to_string();
                if !name.is_empty() {
                    let _ = DEVICE_NAME.set(name);
                }
                if !info.download_dir.is_empty() {
                    let _ = PUBLIC_SAVE_DIR.set(PathBuf::from(info.download_dir));
                }
            }

            #[cfg(target_os = "ios")]
            restore_save_dir(app);

            use tauri_plugin_notification::{NotificationExt, PermissionState};
            let notifications = app.notification();
            if !matches!(
                notifications.permission_state(),
                Ok(PermissionState::Granted)
            ) {
                let _ = notifications.request_permission();
            }
        });
    }

    #[cfg(target_os = "ios")]
    fn restore_save_dir(app: &AppHandle) {
        use tauri::Manager;
        let Some(store) = app.try_state::<crate::settings::store::SettingsStore>() else {
            return;
        };
        if store.get().save_dir.is_none() {
            return;
        }
        let restored = call::<PathResponse>(app, "restoreSaveDir", json!({}))
            .ok()
            .and_then(|r| r.path);
        let _ = store.set_save_dir(restored.map(PathBuf::from));
    }

    pub async fn open_url(app: &AppHandle, url: &str) -> AppResult<()> {
        let app = app.clone();
        let url = url.to_string();
        tokio::task::spawn_blocking(move || {
            use tauri_plugin_opener::OpenerExt;
            app.opener()
                .open_url(url, None::<&str>)
                .map_err(|e| AppError::new(format!("failed to open link: {e}")))
        })
        .await
        .map_err(|e| AppError::new(format!("native call failed: {e}")))?
    }

    pub async fn open_path(app: &AppHandle, path: &str) -> AppResult<()> {
        call_blocking::<Value>(app, "openFile", json!({ "path": path }))
            .await
            .map(|_| ())
    }

    pub async fn reveal_path(app: &AppHandle, path: &str) -> AppResult<()> {
        call_blocking::<Value>(app, "revealFile", json!({ "path": path }))
            .await
            .map(|_| ())
    }

    pub async fn clipboard_image(app: &AppHandle) -> AppResult<PathBuf> {
        let dir = super::new_import_dir(app)?;
        let r: PathResponse = call_blocking(
            app,
            "clipboardImage",
            json!({ "dir": dir.to_string_lossy() }),
        )
        .await?;
        match r.path {
            Some(p) => Ok(PathBuf::from(p)),
            None => {
                let _ = std::fs::remove_dir_all(&dir);
                Err(AppError::new("no image in the clipboard"))
            }
        }
    }

    pub fn files_saved(app: &AppHandle, paths: &[PathBuf]) {
        if !cfg!(target_os = "android") || paths.is_empty() {
            return;
        }
        let paths: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        detached(app, move |app| {
            let _ = call::<Value>(app, "mediaScan", json!({ "paths": paths }));
        });
    }

    pub fn set_background_service(app: &AppHandle, enabled: bool) {
        detached(app, move |app| {
            let _ = call::<Value>(
                app,
                "setBackgroundService",
                json!({
                    "enabled": enabled,
                    "title": "ZWormHole is ready to receive",
                    "text": "Paired devices can send you files while the app is in the background.",
                }),
            );
        });
    }

    pub async fn pick_files(app: &AppHandle, images: bool) -> AppResult<Vec<String>> {
        let dir = super::new_import_dir(app)?;
        let r: PathsResponse = call_blocking(
            app,
            "pickFiles",
            json!({ "dir": dir.to_string_lossy(), "multiple": !images, "images": images }),
        )
        .await?;
        if r.cancelled || r.paths.is_empty() {
            let _ = std::fs::remove_dir_all(&dir);
        }
        Ok(r.paths)
    }

    pub async fn pick_folder(app: &AppHandle) -> AppResult<Option<String>> {
        let dir = super::new_import_dir(app)?;
        let r: PathResponse =
            call_blocking(app, "pickFolder", json!({ "dir": dir.to_string_lossy() })).await?;
        if r.cancelled || r.path.is_none() {
            let _ = std::fs::remove_dir_all(&dir);
        }
        Ok(r.path)
    }

    pub async fn pick_save_dir(app: &AppHandle, persist: bool) -> AppResult<Option<String>> {
        let r: PathResponse =
            call_blocking(app, "pickSaveDir", json!({ "persist": persist })).await?;
        if r.cancelled {
            return Ok(None);
        }
        Ok(r.path)
    }

    pub fn forget_save_dir(app: &AppHandle) {
        if cfg!(target_os = "ios") {
            detached(app, |app| {
                let _ = call::<Value>(app, "clearSaveDir", json!({}));
            });
        }
    }

    pub async fn export_file(app: &AppHandle, source: PathBuf, mime: &str) -> AppResult<bool> {
        let name = source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "export.txt".to_string());
        let r: SavedResponse = call_blocking(
            app,
            "exportFile",
            json!({ "source": source.to_string_lossy(), "name": name, "mime": mime }),
        )
        .await?;
        Ok(r.saved)
    }

    #[cfg(target_os = "ios")]
    pub mod bonjour {
        use std::collections::HashMap;

        use serde::Deserialize;
        use serde_json::{json, Value};
        use tauri::AppHandle;

        use super::call;
        use crate::error::AppResult;

        #[derive(Debug, Clone, Default, Deserialize)]
        #[serde(default)]
        pub struct Service {
            pub fullname: String,
            pub addresses: Vec<String>,
            pub port: u16,
            pub txt: HashMap<String, String>,
        }

        #[derive(Debug, Default, Deserialize)]
        #[serde(default)]
        struct Snapshot {
            services: Vec<Service>,
        }

        pub fn publish(
            app: &AppHandle,
            name: &str,
            port: u16,
            txt: &HashMap<String, String>,
        ) -> AppResult<()> {
            call::<Value>(
                app,
                "bonjourPublish",
                json!({ "name": name, "port": port, "txt": txt }),
            )
            .map(|_| ())
        }

        pub fn unpublish(app: &AppHandle) -> AppResult<()> {
            call::<Value>(app, "bonjourUnpublish", json!({})).map(|_| ())
        }

        pub fn browse(app: &AppHandle) -> AppResult<()> {
            call::<Value>(app, "bonjourBrowse", json!({})).map(|_| ())
        }

        pub fn stop_browse(app: &AppHandle) -> AppResult<()> {
            call::<Value>(app, "bonjourStopBrowse", json!({})).map(|_| ())
        }

        pub fn snapshot(app: &AppHandle) -> AppResult<Vec<Service>> {
            call::<Snapshot>(app, "bonjourSnapshot", json!({})).map(|s| s.services)
        }
    }
}
