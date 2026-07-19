use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedDevice {
    pub fingerprint: String,
    pub display_name: String,
    pub added_at: String,
    #[serde(default)]
    pub expires_at: Option<u64>,


    #[serde(default)]
    pub allow_connection_tests: bool,


    #[serde(default)]
    pub nickname: Option<String>,

    #[serde(default)]
    pub favorite: bool,


    #[serde(default)]
    pub auto_accept: bool,
}

impl TrustedDevice {
    pub fn is_expired(&self, now_unix_secs: u64) -> bool {
        match self.expires_at {
            Some(expires_at) => now_unix_secs >= expires_at,
            None => false,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct TrustedDeviceFile {
    devices: Vec<TrustedDevice>,
}


pub struct TrustedStore {
    path: PathBuf,
}

impl TrustedStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn read(&self) -> AppResult<TrustedDeviceFile> {
        if !self.path.exists() {
            return Ok(TrustedDeviceFile::default());
        }
        let raw = fs::read_to_string(&self.path)
            .map_err(|e| AppError::new(format!("failed to read trusted store: {e}")))?;
        serde_json::from_str(&raw)
            .map_err(|e| AppError::new(format!("failed to parse trusted store: {e}")))
    }

    fn write(&self, file: &TrustedDeviceFile) -> AppResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::new(format!("failed to create app data dir: {e}")))?;
        }
        let raw = serde_json::to_string_pretty(file)
            .map_err(|e| AppError::new(format!("failed to serialize trusted store: {e}")))?;
        fs::write(&self.path, raw)
            .map_err(|e| AppError::new(format!("failed to write trusted store: {e}")))
    }

    pub fn list(&self) -> AppResult<Vec<TrustedDevice>> {
        Ok(self.read()?.devices)
    }

    pub fn get(&self, fingerprint: &str) -> AppResult<Option<TrustedDevice>> {
        Ok(self
            .read()?
            .devices
            .into_iter()
            .find(|d| d.fingerprint == fingerprint))
    }


    pub fn upsert(&self, device: TrustedDevice) -> AppResult<()> {
        let mut file = self.read()?;
        file.devices.retain(|d| d.fingerprint != device.fingerprint);
        file.devices.push(device);
        self.write(&file)
    }

    pub fn revoke(&self, fingerprint: &str) -> AppResult<()> {
        let mut file = self.read()?;
        file.devices.retain(|d| d.fingerprint != fingerprint);
        self.write(&file)
    }


    pub fn set_allow_connection_tests(&self, fingerprint: &str, allow: bool) -> AppResult<()> {
        let mut file = self.read()?;
        if let Some(device) = file.devices.iter_mut().find(|d| d.fingerprint == fingerprint) {
            device.allow_connection_tests = allow;
            self.write(&file)?;
        }
        Ok(())
    }


    pub fn set_nickname(&self, fingerprint: &str, nickname: Option<String>) -> AppResult<()> {
        let mut file = self.read()?;
        if let Some(device) = file.devices.iter_mut().find(|d| d.fingerprint == fingerprint) {


            device.nickname = nickname.filter(|n| !n.trim().is_empty());
            self.write(&file)?;
        }
        Ok(())
    }


    pub fn set_favorite(&self, fingerprint: &str, favorite: bool) -> AppResult<()> {
        let mut file = self.read()?;
        if let Some(device) = file.devices.iter_mut().find(|d| d.fingerprint == fingerprint) {
            device.favorite = favorite;
            self.write(&file)?;
        }
        Ok(())
    }


    pub fn set_auto_accept(&self, fingerprint: &str, auto_accept: bool) -> AppResult<()> {
        let mut file = self.read()?;
        if let Some(device) = file.devices.iter_mut().find(|d| d.fingerprint == fingerprint) {
            device.auto_accept = auto_accept;
            self.write(&file)?;
        }
        Ok(())
    }
}

pub fn default_store_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("trusted_devices.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (TrustedStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = TrustedStore::new(dir.path().join("trusted_devices.json"));
        (store, dir)
    }

    #[test]
    fn round_trips_a_device() {
        let (store, _dir) = temp_store();

        store
            .upsert(TrustedDevice {
                fingerprint: "abc123".into(),
                display_name: "MyDesktop".into(),
                added_at: "2026-07-14T00:00:00Z".into(),
                expires_at: None,
                allow_connection_tests: false,
                nickname: None,
                favorite: false,
                auto_accept: false,
            })
            .unwrap();

        let found = store.get("abc123").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().display_name, "MyDesktop");
    }

    #[test]
    fn revoke_removes_the_device() {
        let (store, _dir) = temp_store();

        store
            .upsert(TrustedDevice {
                fingerprint: "abc123".into(),
                display_name: "MyDesktop".into(),
                added_at: "2026-07-14T00:00:00Z".into(),
                expires_at: None,
                allow_connection_tests: false,
                nickname: None,
                favorite: false,
                auto_accept: false,
            })
            .unwrap();
        store.revoke("abc123").unwrap();

        assert!(store.get("abc123").unwrap().is_none());
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn upsert_replaces_existing_entry_for_same_fingerprint() {
        let (store, _dir) = temp_store();

        store
            .upsert(TrustedDevice {
                fingerprint: "abc123".into(),
                display_name: "Old Name".into(),
                added_at: "2026-07-14T00:00:00Z".into(),
                expires_at: None,
                allow_connection_tests: false,
                nickname: None,
                favorite: false,
                auto_accept: false,
            })
            .unwrap();
        store
            .upsert(TrustedDevice {
                fingerprint: "abc123".into(),
                display_name: "New Name".into(),
                added_at: "2026-07-14T00:00:01Z".into(),
                expires_at: None,
                allow_connection_tests: false,
                nickname: None,
                favorite: false,
                auto_accept: false,
            })
            .unwrap();

        let all = store.list().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].display_name, "New Name");
    }

    #[test]
    fn expiry_is_computed_relative_to_the_given_time() {
        let never = TrustedDevice {
            fingerprint: "a".into(),
            display_name: "Never".into(),
            added_at: "now".into(),
            expires_at: None,
            allow_connection_tests: false,
            nickname: None,
            favorite: false,
            auto_accept: false,
        };
        assert!(!never.is_expired(u64::MAX));

        let expiring = TrustedDevice {
            fingerprint: "b".into(),
            display_name: "Expiring".into(),
            added_at: "now".into(),
            expires_at: Some(1000),
            allow_connection_tests: false,
            nickname: None,
            favorite: false,
            auto_accept: false,
        };
        assert!(!expiring.is_expired(999));
        assert!(expiring.is_expired(1000));
        assert!(expiring.is_expired(1001));
    }
}
