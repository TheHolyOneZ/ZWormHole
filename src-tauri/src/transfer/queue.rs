use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferStatus {
    Queued,


    Archiving,
    InProgress,


    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl TransferStatus {


    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            TransferStatus::Completed | TransferStatus::Failed | TransferStatus::Cancelled
        )
    }
}

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferJob {
    pub id: String,
    pub device_fullname: String,


    pub label: String,
    pub status: TransferStatus,
    pub bytes_total: u64,
    pub bytes_done: u64,

    pub created_at: u64,


    pub local_micros: u64,
    pub encrypt_micros: u64,
    pub socket_micros: u64,


    pub peer_recv_wait_micros: u64,
    pub peer_disk_write_micros: u64,
}

impl TransferJob {
    pub fn new(device_fullname: String, bytes_total: u64, label: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            device_fullname,
            label,
            status: TransferStatus::Queued,
            bytes_total,
            bytes_done: 0,
            created_at: now_unix_secs(),
            local_micros: 0,
            encrypt_micros: 0,
            socket_micros: 0,
            peer_recv_wait_micros: 0,
            peer_disk_write_micros: 0,
        }
    }
}


#[derive(Default)]
pub struct TransferQueue {
    jobs: HashMap<String, TransferJob>,


    cancels: HashMap<String, Arc<AtomicBool>>,


    pauses: HashMap<String, Arc<AtomicBool>>,
}

impl TransferQueue {
    pub fn enqueue(&mut self, device_fullname: String, bytes_total: u64, label: String) -> TransferJob {
        let job = TransferJob::new(device_fullname, bytes_total, label);
        self.jobs.insert(job.id.clone(), job.clone());
        job
    }

    pub fn get(&self, id: &str) -> Option<&TransferJob> {
        self.jobs.get(id)
    }

    pub fn list(&self) -> Vec<TransferJob> {
        self.jobs.values().cloned().collect()
    }

    pub fn set_status(&mut self, id: &str, status: TransferStatus) -> bool {
        match self.jobs.get_mut(id) {
            Some(job) => {
                job.status = status;
                true
            }
            None => false,
        }
    }

    pub fn set_progress(&mut self, id: &str, bytes_done: u64) -> bool {
        match self.jobs.get_mut(id) {
            Some(job) => {
                job.bytes_done = bytes_done;
                true
            }
            None => false,
        }
    }


    pub fn cancel_flag(&mut self, id: &str) -> Arc<AtomicBool> {
        self.cancels
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone()
    }


    pub fn clear_cancel(&mut self, id: &str) {
        self.cancels.remove(id);
        self.pauses.remove(id);
    }


    pub fn pause_flag(&mut self, id: &str) -> Arc<AtomicBool> {
        self.pauses
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone()
    }


    pub fn set_timing(
        &mut self,
        id: &str,
        local_micros: u64,
        encrypt_micros: u64,
        socket_micros: u64,
    ) -> bool {
        match self.jobs.get_mut(id) {
            Some(job) => {
                job.local_micros = local_micros;
                job.encrypt_micros = encrypt_micros;
                job.socket_micros = socket_micros;
                true
            }
            None => false,
        }
    }


    pub fn set_peer_timing(
        &mut self,
        id: &str,
        peer_recv_wait_micros: u64,
        peer_disk_write_micros: u64,
    ) -> bool {
        match self.jobs.get_mut(id) {
            Some(job) => {
                job.peer_recv_wait_micros = peer_recv_wait_micros;
                job.peer_disk_write_micros = peer_disk_write_micros;
                true
            }
            None => false,
        }
    }


    pub fn set_total(&mut self, id: &str, bytes_total: u64) -> bool {
        match self.jobs.get_mut(id) {
            Some(job) => {
                job.bytes_total = bytes_total;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_then_progress_then_cancel() {
        let mut queue = TransferQueue::default();
        let job = queue.enqueue("MyDesktop._zwormhole._tcp.local.".into(), 1024, "report.pdf".into());

        assert_eq!(queue.get(&job.id).unwrap().status, TransferStatus::Queued);

        assert!(queue.set_status(&job.id, TransferStatus::InProgress));
        assert!(queue.set_progress(&job.id, 512));
        assert_eq!(queue.get(&job.id).unwrap().bytes_done, 512);

        assert!(queue.set_status(&job.id, TransferStatus::Cancelled));
        assert_eq!(queue.get(&job.id).unwrap().status, TransferStatus::Cancelled);
    }

    #[test]
    fn unknown_job_id_is_a_no_op() {
        let mut queue = TransferQueue::default();
        assert!(!queue.set_status("nonexistent", TransferStatus::Failed));
        assert!(!queue.set_progress("nonexistent", 10));
    }
}
