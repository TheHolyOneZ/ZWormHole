

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedDevice {
    pub fingerprint: String,


    pub display_name: String,


    pub blocked_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct BlockedFile {
    devices: Vec<BlockedDevice>,
}


pub struct BlockStore {
    path: PathBuf,
}

impl BlockStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn read(&self) -> AppResult<BlockedFile> {
        if !self.path.exists() {
            return Ok(BlockedFile::default());
        }
        let raw = fs::read_to_string(&self.path)
            .map_err(|e| AppError::new(format!("failed to read blocklist: {e}")))?;
        serde_json::from_str(&raw)
            .map_err(|e| AppError::new(format!("failed to parse blocklist: {e}")))
    }

    fn write(&self, file: &BlockedFile) -> AppResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::new(format!("failed to create app data dir: {e}")))?;
        }
        let raw = serde_json::to_string_pretty(file)
            .map_err(|e| AppError::new(format!("failed to serialize blocklist: {e}")))?;
        fs::write(&self.path, raw)
            .map_err(|e| AppError::new(format!("failed to write blocklist: {e}")))
    }

    pub fn list(&self) -> AppResult<Vec<BlockedDevice>> {
        Ok(self.read()?.devices)
    }


    pub fn is_blocked(&self, fingerprint: &str) -> bool {
        self.read()
            .map(|f| f.devices.iter().any(|d| d.fingerprint == fingerprint))
            .unwrap_or(false)
    }


    pub fn block(&self, device: BlockedDevice) -> AppResult<()> {
        let mut file = self.read()?;
        file.devices.retain(|d| d.fingerprint != device.fingerprint);
        file.devices.push(device);
        self.write(&file)
    }

    pub fn unblock(&self, fingerprint: &str) -> AppResult<()> {
        let mut file = self.read()?;
        file.devices.retain(|d| d.fingerprint != fingerprint);
        self.write(&file)
    }
}

pub fn default_store_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("blocked_devices.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (BlockStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = BlockStore::new(dir.path().join("blocked_devices.json"));
        (store, dir)
    }

    fn dev(fp: &str) -> BlockedDevice {
        BlockedDevice {
            fingerprint: fp.into(),
            display_name: "Someone".into(),
            blocked_at: "0".into(),
        }
    }

    #[test]
    fn blocks_and_reports_membership() {
        let (store, _dir) = temp_store();
        assert!(!store.is_blocked("abc"));
        store.block(dev("abc")).unwrap();
        assert!(store.is_blocked("abc"));
        assert!(!store.is_blocked("def"));
    }

    #[test]
    fn block_is_idempotent() {
        let (store, _dir) = temp_store();
        store.block(dev("abc")).unwrap();
        store.block(dev("abc")).unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
    }

    #[test]
    fn unblock_removes_it() {
        let (store, _dir) = temp_store();
        store.block(dev("abc")).unwrap();
        store.unblock("abc").unwrap();
        assert!(!store.is_blocked("abc"));
        assert!(store.list().unwrap().is_empty());
    }
}
