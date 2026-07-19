use std::collections::HashMap;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::error::{AppError, AppResult};

pub const SERVICE_TYPE: &str = "_zwormhole._tcp.local.";

const TXT_HOSTNAME: &str = "hostname";
const TXT_SESSION_ID: &str = "session_id";
const TXT_FINGERPRINT: &str = "fingerprint";
const TXT_PROTOCOL_VERSION: &str = "protocol_version";
const TXT_OS: &str = "os";

pub const PROTOCOL_VERSION: &str = "1";


#[derive(Debug, Clone)]
pub struct LocalDeviceInfo {
    pub display_name: String,
    pub session_id: String,
    pub fingerprint: String,
    pub port: u16,
}


#[derive(Debug, Clone)]
pub struct NearbyDevice {
    pub fullname: String,
    pub hostname: String,
    pub session_id: String,
    pub fingerprint: String,
    pub protocol_version: String,
    pub os: String,
    pub addresses: Vec<std::net::IpAddr>,
    pub port: u16,
}

pub fn current_os_badge() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}


pub fn advertise(daemon: &ServiceDaemon, info: &LocalDeviceInfo) -> AppResult<String> {


    let stable_id = &info.fingerprint[..info.fingerprint.len().min(32)];
    let host_name = format!("{stable_id}.local.");

    let mut properties: HashMap<String, String> = HashMap::new();
    properties.insert(TXT_HOSTNAME.to_string(), info.display_name.clone());
    properties.insert(TXT_SESSION_ID.to_string(), info.session_id.clone());
    properties.insert(TXT_FINGERPRINT.to_string(), info.fingerprint.clone());
    properties.insert(
        TXT_PROTOCOL_VERSION.to_string(),
        PROTOCOL_VERSION.to_string(),
    );
    properties.insert(TXT_OS.to_string(), current_os_badge().to_string());

    let service = ServiceInfo::new(
        SERVICE_TYPE,
        stable_id,
        &host_name,
        "",
        info.port,
        properties,
    )
    .map_err(|e| AppError::new(format!("failed to build mDNS service info: {e}")))?
    .enable_addr_auto();

    let fullname = service.get_fullname().to_string();

    daemon
        .register(service)
        .map_err(|e| AppError::new(format!("failed to register mDNS service: {e}")))?;

    Ok(fullname)
}

pub fn stop_advertising(daemon: &ServiceDaemon, fullname: &str) -> AppResult<()> {
    daemon
        .unregister(fullname)
        .map_err(|e| AppError::new(format!("failed to unregister mDNS service: {e}")))?;
    Ok(())
}


pub fn browse(daemon: &ServiceDaemon) -> AppResult<mdns_sd::Receiver<ServiceEvent>> {
    daemon
        .browse(SERVICE_TYPE)
        .map_err(|e| AppError::new(format!("failed to browse for mDNS services: {e}")))
}

pub fn stop_browsing(daemon: &ServiceDaemon) -> AppResult<()> {
    daemon
        .stop_browse(SERVICE_TYPE)
        .map_err(|e| AppError::new(format!("failed to stop mDNS browse: {e}")))
}


pub fn decode_resolved(resolved: &mdns_sd::ResolvedService) -> Option<NearbyDevice> {
    let hostname = resolved.txt_properties.get_property_val_str(TXT_HOSTNAME)?;
    let session_id = resolved
        .txt_properties
        .get_property_val_str(TXT_SESSION_ID)?;
    let fingerprint = resolved
        .txt_properties
        .get_property_val_str(TXT_FINGERPRINT)?;
    let protocol_version = resolved
        .txt_properties
        .get_property_val_str(TXT_PROTOCOL_VERSION)
        .unwrap_or("unknown");
    let os = resolved
        .txt_properties
        .get_property_val_str(TXT_OS)
        .unwrap_or("unknown");

    Some(NearbyDevice {
        fullname: resolved.fullname.clone(),
        hostname: hostname.to_string(),
        session_id: session_id.to_string(),
        fingerprint: fingerprint.to_string(),
        protocol_version: protocol_version.to_string(),
        os: os.to_string(),
        addresses: resolved.addresses.iter().map(|a| a.to_ip_addr()).collect(),
        port: resolved.port,
    })
}
