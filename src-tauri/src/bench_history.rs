

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchHistoryEntry {
    pub id: String,
    pub fingerprint: String,
    pub display_name: String,
    pub at_unix: u64,

    pub up_avg_mbps: f64,

    pub down_avg_mbps: f64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct BenchFile {
    entries: Vec<BenchHistoryEntry>,
}

const MAX_ENTRIES: usize = 100;

pub struct BenchHistoryStore {
    path: PathBuf,
}

impl BenchHistoryStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn read(&self) -> AppResult<BenchFile> {
        if !self.path.exists() {
            return Ok(BenchFile::default());
        }
        let raw = fs::read_to_string(&self.path)
            .map_err(|e| AppError::new(format!("failed to read benchmark history: {e}")))?;
        serde_json::from_str(&raw)
            .map_err(|e| AppError::new(format!("failed to parse benchmark history: {e}")))
    }

    fn write(&self, file: &BenchFile) -> AppResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::new(format!("failed to create app data dir: {e}")))?;
        }
        let raw = serde_json::to_string_pretty(file)
            .map_err(|e| AppError::new(format!("failed to serialize benchmark history: {e}")))?;
        fs::write(&self.path, raw)
            .map_err(|e| AppError::new(format!("failed to write benchmark history: {e}")))
    }

    pub fn record(&self, entry: BenchHistoryEntry) -> AppResult<()> {
        let mut file = self.read()?;
        file.entries.push(entry);
        let len = file.entries.len();
        if len > MAX_ENTRIES {
            file.entries.drain(0..len - MAX_ENTRIES);
        }
        self.write(&file)
    }


    pub fn list(&self) -> AppResult<Vec<BenchHistoryEntry>> {
        let mut entries = self.read()?.entries;
        entries.reverse();
        Ok(entries)
    }

    pub fn clear(&self) -> AppResult<()> {
        self.write(&BenchFile::default())
    }
}

pub fn default_store_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("benchmark_history.json")
}
