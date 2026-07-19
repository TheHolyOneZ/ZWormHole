

use crate::error::{AppError, AppResult};


#[tauri::command]
pub fn open_external_url(url: String) -> AppResult<()> {
    if !url.starts_with("https://") {
        return Err(AppError::new("refusing to open a non-https URL"));
    }
    if url.len() > 2048
        || url.contains(|c: char| "\"'\n\r\0&|;`<>^%$".contains(c) || c.is_control())
    {
        return Err(AppError::new("URL contains characters that aren't allowed"));
    }
    open_url_native(&url)
}

#[cfg(target_os = "linux")]
fn open_url_native(url: &str) -> AppResult<()> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::new(format!("failed to launch xdg-open: {e}")))
}


#[tauri::command]
pub fn save_text_file(path: String, contents: String) -> AppResult<()> {
    std::fs::write(&path, &contents)
        .map_err(|e| AppError::new(format!("failed to write {path}: {e}")))
}


#[tauri::command]
pub fn open_path(path: String) -> AppResult<()> {
    if !std::path::Path::new(&path).exists() {
        return Err(AppError::new("that file no longer exists on disk"));
    }
    open_path_native(&path)
}


#[tauri::command]
pub fn reveal_path(path: String) -> AppResult<()> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(AppError::new("that file no longer exists on disk"));
    }
    reveal_path_native(&path)
}

#[cfg(target_os = "linux")]
fn open_path_native(path: &str) -> AppResult<()> {
    std::process::Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::new(format!("failed to open file: {e}")))
}

#[cfg(target_os = "linux")]
fn reveal_path_native(path: &str) -> AppResult<()> {

    let dir = std::path::Path::new(path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    std::process::Command::new("xdg-open")
        .arg(dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::new(format!("failed to open folder: {e}")))
}

#[cfg(target_os = "macos")]
fn open_path_native(path: &str) -> AppResult<()> {
    std::process::Command::new("open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::new(format!("failed to open file: {e}")))
}

#[cfg(target_os = "macos")]
fn reveal_path_native(path: &str) -> AppResult<()> {
    std::process::Command::new("open")
        .args(["-R", path])
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::new(format!("failed to reveal file: {e}")))
}

#[cfg(target_os = "windows")]
fn open_path_native(path: &str) -> AppResult<()> {
    use std::os::windows::process::CommandExt;


    std::process::Command::new("cmd")
        .raw_arg(format!("/C start \"\" \"{path}\""))
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::new(format!("failed to open file: {e}")))
}

#[cfg(target_os = "windows")]
fn reveal_path_native(path: &str) -> AppResult<()> {
    use std::os::windows::process::CommandExt;


    std::process::Command::new("explorer")
        .raw_arg(format!("/select,\"{path}\""))
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::new(format!("failed to reveal file: {e}")))
}

#[cfg(target_os = "macos")]
fn open_url_native(url: &str) -> AppResult<()> {
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::new(format!("failed to launch open: {e}")))
}

#[cfg(target_os = "windows")]
fn open_url_native(url: &str) -> AppResult<()> {


    std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::new(format!("failed to launch start: {e}")))
}
