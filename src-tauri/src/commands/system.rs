use tauri::AppHandle;

use crate::error::{AppError, AppResult};
use crate::platform::{self, PlatformInfo};

#[tauri::command]
pub fn platform_info() -> PlatformInfo {
    platform::info()
}

#[tauri::command]
pub async fn open_external_url(app: AppHandle, url: String) -> AppResult<()> {
    if !url.starts_with("https://") {
        return Err(AppError::new("refusing to open a non-https URL"));
    }
    if url.len() > 2048
        || url.contains(|c: char| "\"'\n\r\0&|;`<>^%$".contains(c) || c.is_control())
    {
        return Err(AppError::new("URL contains characters that aren't allowed"));
    }
    platform::open_url(&app, &url).await
}

#[tauri::command]
pub fn save_text_file(path: String, contents: String) -> AppResult<()> {
    std::fs::write(&path, &contents)
        .map_err(|e| AppError::new(format!("failed to write {path}: {e}")))
}

#[tauri::command]
pub async fn open_path(app: AppHandle, path: String) -> AppResult<()> {
    if !std::path::Path::new(&path).exists() {
        return Err(AppError::new("that file no longer exists on disk"));
    }
    platform::open_path(&app, &path).await
}

#[tauri::command]
pub async fn reveal_path(app: AppHandle, path: String) -> AppResult<()> {
    if !std::path::Path::new(&path).exists() {
        return Err(AppError::new("that file no longer exists on disk"));
    }
    platform::reveal_path(&app, &path).await
}

#[tauri::command]
pub async fn pick_files(app: AppHandle, images: Option<bool>) -> AppResult<Vec<String>> {
    platform::pick_files(&app, images.unwrap_or(false)).await
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> AppResult<Option<String>> {
    platform::pick_folder(&app).await
}

#[tauri::command]
pub async fn pick_save_dir(app: AppHandle, persist: Option<bool>) -> AppResult<Option<String>> {
    let Some(dir) = platform::pick_save_dir(&app, persist.unwrap_or(true)).await? else {
        return Ok(None);
    };
    platform::ensure_writable_dir(std::path::Path::new(&dir))?;
    Ok(Some(dir))
}

fn export_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) || c.is_control() { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        "export.txt".to_string()
    } else {
        cleaned
    }
}

#[tauri::command]
pub async fn export_text_file(
    app: AppHandle,
    file_name: String,
    contents: String,
    mime: Option<String>,
) -> AppResult<bool> {
    let dir = platform::new_import_dir(&app)?;
    let source = dir.join(export_name(&file_name));
    std::fs::write(&source, contents.as_bytes())
        .map_err(|e| AppError::new(format!("failed to prepare export: {e}")))?;
    let result = platform::export_file(
        &app,
        source,
        mime.as_deref().unwrap_or("text/plain"),
    )
    .await;
    let _ = std::fs::remove_dir_all(&dir);
    result
}
