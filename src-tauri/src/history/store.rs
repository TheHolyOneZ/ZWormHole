use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,

    pub device_key: String,

    pub display_name: String,
    pub file_names: Vec<String>,

    pub direction: String,
    pub total_bytes: u64,

    pub at_unix: u64,


    #[serde(default)]
    pub paths: Vec<String>,


    #[serde(default)]
    pub rate_samples: Vec<u32>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct HistoryFile {
    entries: Vec<HistoryEntry>,
}


const MAX_ENTRIES: usize = 500;


pub struct HistoryStore {
    path: PathBuf,
}

impl HistoryStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn read(&self) -> AppResult<HistoryFile> {
        if !self.path.exists() {
            return Ok(HistoryFile::default());
        }
        let raw = fs::read_to_string(&self.path)
            .map_err(|e| AppError::new(format!("failed to read history store: {e}")))?;
        serde_json::from_str(&raw)
            .map_err(|e| AppError::new(format!("failed to parse history store: {e}")))
    }

    fn write(&self, file: &HistoryFile) -> AppResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::new(format!("failed to create app data dir: {e}")))?;
        }
        let raw = serde_json::to_string_pretty(file)
            .map_err(|e| AppError::new(format!("failed to serialize history store: {e}")))?;
        fs::write(&self.path, raw)
            .map_err(|e| AppError::new(format!("failed to write history store: {e}")))
    }


    pub fn record(&self, entry: HistoryEntry) -> AppResult<()> {
        let mut file = self.read()?;
        file.entries.push(entry);
        let len = file.entries.len();
        if len > MAX_ENTRIES {
            file.entries.drain(0..len - MAX_ENTRIES);
        }
        self.write(&file)
    }


    pub fn list(&self) -> AppResult<Vec<HistoryEntry>> {
        let mut entries = self.read()?.entries;
        entries.reverse();
        Ok(entries)
    }

    pub fn clear(&self) -> AppResult<()> {
        self.write(&HistoryFile::default())
    }
}

pub fn default_store_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("transfer_history.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (HistoryStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (HistoryStore::new(dir.path().join("history.json")), dir)
    }

    fn entry(id: &str, at: u64) -> HistoryEntry {
        HistoryEntry {
            id: id.to_string(),
            device_key: "fp".into(),
            display_name: "Peer".into(),
            file_names: vec!["a.bin".into()],
            direction: "sent".into(),
            total_bytes: 10,
            at_unix: at,
            paths: vec![],
            rate_samples: vec![],
        }
    }

    #[test]
    fn records_and_lists_newest_first() {
        let (store, _dir) = temp_store();
        store.record(entry("1", 100)).unwrap();
        store.record(entry("2", 200)).unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "2");
        assert_eq!(list[1].id, "1");
    }

    #[test]
    fn trims_to_the_cap() {
        let (store, _dir) = temp_store();
        for i in 0..(MAX_ENTRIES + 20) {
            store.record(entry(&i.to_string(), i as u64)).unwrap();
        }
        let list = store.list().unwrap();
        assert_eq!(list.len(), MAX_ENTRIES);

        assert_eq!(list[0].id, (MAX_ENTRIES + 19).to_string());
    }

    #[test]
    fn clear_empties_the_store() {
        let (store, _dir) = temp_store();
        store.record(entry("1", 1)).unwrap();
        store.clear().unwrap();
        assert!(store.list().unwrap().is_empty());
    }
}
