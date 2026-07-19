

pub mod discovery;
pub mod pairing;
pub mod profile;
pub mod settings;
pub mod system;
pub mod transfer;
pub mod wan;

use tauri::{AppHandle, Emitter};

use crate::events::{PongPayload, PING_PONG};


#[tauri::command]
pub fn ping(app: AppHandle, message: String) -> Result<(), String> {
    app.emit(PING_PONG, PongPayload { echo: message })
        .map_err(|e| e.to_string())
}
