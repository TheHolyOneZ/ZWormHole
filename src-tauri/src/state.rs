

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize};

use mdns_sd::ServiceDaemon;
use quinn::Endpoint;
use tokio::sync::{oneshot, Mutex, Notify};
use tokio::task::JoinHandle;

use crate::discovery::service::NearbyDevice;
use crate::net::{BenchStats, IncomingDecision, PeerConnection, PendingInboundRequest, SpeedTestReply};
use crate::transfer::queue::TransferQueue;

#[derive(Default)]
pub struct DiscoveryState {
    pub daemon: Option<ServiceDaemon>,


    pub own_fullname: Option<String>,

    pub nearby: HashMap<String, NearbyDevice>,


    pub browse_task: Option<JoinHandle<()>>,
}

#[derive(Default)]
pub struct NetState {
    pub endpoint: Option<Endpoint>,
    pub inbound_pending: HashMap<String, PendingInboundRequest>,
    pub peers: HashMap<String, PeerConnection>,


    pub pending_incoming: HashMap<String, oneshot::Sender<IncomingDecision>>,


    pub pending_speedtests: HashMap<String, oneshot::Sender<SpeedTestReply>>,


    pub pending_consent_replies: HashMap<String, oneshot::Sender<bool>>,


    pub pending_consent_prompts: HashMap<String, oneshot::Sender<(bool, bool)>>,


    pub test_allowances: HashMap<String, std::time::Instant>,


    pub pending_bench_results: HashMap<String, oneshot::Sender<BenchStats>>,


    pub pending_bench_pulls: HashMap<String, oneshot::Sender<BenchStats>>,
}


#[derive(Default)]
pub struct WanState {

    pub mapping: Option<crate::portmap::WanMapping>,


    pub generation: u64,
}

#[derive(Default)]
pub struct AppState {
    pub discovery: Mutex<DiscoveryState>,
    pub net: Mutex<NetState>,
    pub transfer_queue: Mutex<TransferQueue>,


    pub background_listener_enabled: AtomicBool,

    pub wan: Mutex<WanState>,


    pub active_sends: AtomicUsize,
    pub send_slot_freed: Notify,
}
