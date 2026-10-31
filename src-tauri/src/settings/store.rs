use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};


#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoExtractMode {
    #[default]
    Off,
    ExtractAndDelete,
    ExtractAndKeep,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferSettings {
    #[serde(default)]
    pub auto_accept: bool,
    #[serde(default)]
    pub save_dir: Option<PathBuf>,
    #[serde(default)]
    pub auto_extract: AutoExtractMode,


    #[serde(default)]
    pub max_send_mb_s: u32,


    #[serde(default)]
    pub max_concurrent_sends: u32,


    #[serde(default)]
    pub quic_port: u16,

    #[serde(default)]
    pub background_listener: bool,
}

impl Default for TransferSettings {
    fn default() -> Self {
        Self {
            auto_accept: false,
            save_dir: None,
            auto_extract: AutoExtractMode::default(),
            max_send_mb_s: 0,
            max_concurrent_sends: 0,
            quic_port: 0,
            background_listener: false,
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
    cache: RwLock<TransferSettings>,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        let cache = Self::read_from_disk(&path).unwrap_or_default();
        Self {
            path,
            cache: RwLock::new(cache),
        }
    }

    fn read_from_disk(path: &Path) -> Option<TransferSettings> {
        let raw = fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn get(&self) -> TransferSettings {
        self.cache.read().unwrap().clone()
    }

    fn write(&self, settings: &TransferSettings) -> AppResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::new(format!("failed to create app data dir: {e}")))?;
        }
        let raw = serde_json::to_string_pretty(settings)
            .map_err(|e| AppError::new(format!("failed to serialize settings: {e}")))?;
        fs::write(&self.path, raw)
            .map_err(|e| AppError::new(format!("failed to write settings: {e}")))
    }

    pub fn set_auto_accept(&self, enabled: bool) -> AppResult<()> {
        let mut settings = self.get();
        settings.auto_accept = enabled;
        self.write(&settings)?;
        *self.cache.write().unwrap() = settings;
        Ok(())
    }

    pub fn set_background_listener(&self, enabled: bool) -> AppResult<()> {
        let mut settings = self.get();
        settings.background_listener = enabled;
        self.write(&settings)?;
        *self.cache.write().unwrap() = settings;
        Ok(())
    }

    pub fn set_save_dir(&self, dir: Option<PathBuf>) -> AppResult<()> {
        let mut settings = self.get();
        settings.save_dir = dir;
        self.write(&settings)?;
        *self.cache.write().unwrap() = settings;
        Ok(())
    }

    pub fn set_auto_extract(&self, mode: AutoExtractMode) -> AppResult<()> {
        let mut settings = self.get();
        settings.auto_extract = mode;
        self.write(&settings)?;
        *self.cache.write().unwrap() = settings;
        Ok(())
    }

    pub fn set_max_send_mb_s(&self, mb_s: u32) -> AppResult<()> {
        let mut settings = self.get();
        settings.max_send_mb_s = mb_s;
        self.write(&settings)?;
        *self.cache.write().unwrap() = settings;
        Ok(())
    }


    pub fn max_send_mb_s(&self) -> u32 {
        self.cache.read().unwrap().max_send_mb_s
    }

    pub fn set_max_concurrent_sends(&self, n: u32) -> AppResult<()> {
        let mut settings = self.get();
        settings.max_concurrent_sends = n;
        self.write(&settings)?;
        *self.cache.write().unwrap() = settings;
        Ok(())
    }


    pub fn max_concurrent_sends(&self) -> u32 {
        self.cache.read().unwrap().max_concurrent_sends
    }

    pub fn set_quic_port(&self, port: u16) -> AppResult<()> {
        let mut settings = self.get();
        settings.quic_port = port;
        self.write(&settings)?;
        *self.cache.write().unwrap() = settings;
        Ok(())
    }


    pub fn quic_port(&self) -> u16 {
        self.cache.read().unwrap().quic_port
    }
}

pub fn default_settings_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (SettingsStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("settings.json"));
        (store, dir)
    }

    #[test]
    fn defaults_to_auto_accept_off() {
        let (store, _dir) = temp_store();
        let settings = store.get();
        assert!(!settings.auto_accept);
        assert!(settings.save_dir.is_none());
        assert_eq!(settings.auto_extract, AutoExtractMode::Off);
    }

    #[test]
    fn persists_across_reload() {
        let (store, dir) = temp_store();
        store.set_auto_accept(true).unwrap();
        store
            .set_save_dir(Some(PathBuf::from("/tmp/somewhere")))
            .unwrap();
        store
            .set_auto_extract(AutoExtractMode::ExtractAndDelete)
            .unwrap();

        let reloaded = SettingsStore::new(dir.path().join("settings.json"));
        let settings = reloaded.get();
        assert!(settings.auto_accept);
        assert_eq!(settings.save_dir, Some(PathBuf::from("/tmp/somewhere")));
        assert_eq!(settings.auto_extract, AutoExtractMode::ExtractAndDelete);
    }
}
