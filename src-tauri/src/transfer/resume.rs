use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};


#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResumeState {
    pub manifest_id: String,
    pub verified_chunks: HashSet<u32>,
}

pub fn sidecar_path(staging_path: &Path) -> PathBuf {
    let mut p = staging_path.as_os_str().to_owned();
    p.push(".resume.json");
    PathBuf::from(p)
}


pub fn load(staging_path: &Path, manifest_id: &str) -> ResumeState {
    let sidecar = sidecar_path(staging_path);
    let Ok(raw) = std::fs::read_to_string(&sidecar) else {
        return ResumeState::default();
    };
    let Ok(state) = serde_json::from_str::<ResumeState>(&raw) else {
        return ResumeState::default();
    };
    if state.manifest_id != manifest_id {
        return ResumeState::default();
    }
    state
}

pub fn save(staging_path: &Path, state: &ResumeState) -> AppResult<()> {
    let sidecar = sidecar_path(staging_path);
    let raw = serde_json::to_string(state)
        .map_err(|e| AppError::new(format!("failed to serialize resume state: {e}")))?;
    std::fs::write(&sidecar, raw)
        .map_err(|e| AppError::new(format!("failed to write resume sidecar: {e}")))
}

pub fn clear(staging_path: &Path) {
    let _ = std::fs::remove_file(sidecar_path(staging_path));
}
