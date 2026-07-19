use serde::Serialize;

use super::service::NearbyDevice;

pub const DEVICE_APPEARED: &str = "zw://discovery/device-appeared";
pub const DEVICE_UPDATED: &str = "zw://discovery/device-updated";
pub const DEVICE_LOST: &str = "zw://discovery/device-lost";

#[derive(Debug, Clone, Serialize)]
pub struct NearbyDevicePayload {
    pub fullname: String,
    pub hostname: String,
    pub session_id: String,
    pub fingerprint: String,
    pub protocol_version: String,
    pub os: String,
    pub addresses: Vec<String>,
    pub port: u16,
}


impl From<&NearbyDevice> for NearbyDevicePayload {
    fn from(d: &NearbyDevice) -> Self {
        Self {
            fullname: d.fullname.clone(),
            hostname: d.hostname.clone(),
            session_id: d.session_id.clone(),
            fingerprint: d.fingerprint.clone(),
            protocol_version: d.protocol_version.clone(),
            os: d.os.clone(),
            addresses: d.addresses.iter().map(|a| a.to_string()).collect(),
            port: d.port,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceLostPayload {
    pub fullname: String,
}
