

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::transfer::queue::TransferStatus;


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PersistedSource {
    Files { paths: Vec<String> },
    Folder {
        folder_path: String,
        selected_files: Option<Vec<String>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedJob {
    pub id: String,
    pub device_key: String,
    pub source: PersistedSource,
    pub status: TransferStatus,
    pub bytes_total: u64,


    pub updated_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct QueueFile {
    jobs: Vec<PersistedJob>,
}


const MAX_JOBS: usize = 200;


pub struct QueueStore {
    path: PathBuf,
}

impl QueueStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn read(&self) -> AppResult<QueueFile> {
        if !self.path.exists() {
            return Ok(QueueFile::default());
        }
        let raw = fs::read_to_string(&self.path)
            .map_err(|e| AppError::new(format!("failed to read queue store: {e}")))?;
        serde_json::from_str(&raw)
            .map_err(|e| AppError::new(format!("failed to parse queue store: {e}")))
    }

    fn write(&self, file: &QueueFile) -> AppResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::new(format!("failed to create app data dir: {e}")))?;
        }
        let raw = serde_json::to_string_pretty(file)
            .map_err(|e| AppError::new(format!("failed to serialize queue store: {e}")))?;
        fs::write(&self.path, raw)
            .map_err(|e| AppError::new(format!("failed to write queue store: {e}")))
    }


    pub fn upsert(&self, job: PersistedJob) -> AppResult<()> {
        if job.status.is_terminal() {
            return self.remove(&job.id);
        }
        let mut file = self.read()?;
        match file.jobs.iter_mut().find(|j| j.id == job.id) {
            Some(existing) => *existing = job,
            None => file.jobs.push(job),
        }
        let len = file.jobs.len();
        if len > MAX_JOBS {
            file.jobs.drain(0..len - MAX_JOBS);
        }
        self.write(&file)
    }


    pub fn set_status(&self, id: &str, status: TransferStatus, updated_at: u64) -> AppResult<()> {
        if status.is_terminal() {
            return self.remove(id);
        }
        let mut file = self.read()?;
        if let Some(job) = file.jobs.iter_mut().find(|j| j.id == id) {
            job.status = status;
            job.updated_at = updated_at;
            self.write(&file)?;
        }
        Ok(())
    }

    pub fn remove(&self, id: &str) -> AppResult<()> {
        let mut file = self.read()?;
        let before = file.jobs.len();
        file.jobs.retain(|j| j.id != id);
        if file.jobs.len() != before {
            self.write(&file)?;
        }
        Ok(())
    }


    pub fn list(&self) -> AppResult<Vec<PersistedJob>> {
        Ok(self.read()?.jobs)
    }
}

pub fn default_store_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("transfer_queue.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (QueueStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (QueueStore::new(dir.path().join("queue.json")), dir)
    }

    fn job(id: &str, status: TransferStatus) -> PersistedJob {
        PersistedJob {
            id: id.to_string(),
            device_key: "fp".into(),
            source: PersistedSource::Files {
                paths: vec!["/tmp/a.bin".into()],
            },
            status,
            bytes_total: 10,
            updated_at: 1,
        }
    }

    #[test]
    fn upsert_inserts_then_replaces_by_id() {
        let (store, _dir) = temp_store();
        store.upsert(job("1", TransferStatus::Queued)).unwrap();
        store.upsert(job("1", TransferStatus::InProgress)).unwrap();
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].status, TransferStatus::InProgress);
    }

    #[test]
    fn reaching_terminal_removes_the_record() {
        let (store, _dir) = temp_store();
        store.upsert(job("1", TransferStatus::InProgress)).unwrap();
        store
            .set_status("1", TransferStatus::Completed, 2)
            .unwrap();
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn upsert_of_terminal_job_is_dropped() {
        let (store, _dir) = temp_store();
        store.upsert(job("1", TransferStatus::Cancelled)).unwrap();
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn survives_a_reload_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("queue.json");
        {
            let store = QueueStore::new(path.clone());
            store.upsert(job("1", TransferStatus::Paused)).unwrap();
        }

        let reopened = QueueStore::new(path);
        let list = reopened.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "1");
        assert_eq!(list[0].status, TransferStatus::Paused);
    }
}
