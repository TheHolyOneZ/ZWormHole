use std::collections::HashMap;

#[cfg(not(target_os = "ios"))]
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

#[cfg(not(target_os = "ios"))]
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

pub fn instance_name(info: &LocalDeviceInfo) -> String {
    info.fingerprint[..info.fingerprint.len().min(32)].to_string()
}

pub fn txt_properties(info: &LocalDeviceInfo) -> HashMap<String, String> {
    let mut properties: HashMap<String, String> = HashMap::new();
    properties.insert(TXT_HOSTNAME.to_string(), info.display_name.clone());
    properties.insert(TXT_SESSION_ID.to_string(), info.session_id.clone());
    properties.insert(TXT_FINGERPRINT.to_string(), info.fingerprint.clone());
    properties.insert(
        TXT_PROTOCOL_VERSION.to_string(),
        PROTOCOL_VERSION.to_string(),
    );
    properties.insert(TXT_OS.to_string(), crate::platform::os_badge().to_string());
    properties
}

#[cfg(not(target_os = "ios"))]
pub fn advertise(daemon: &ServiceDaemon, info: &LocalDeviceInfo) -> AppResult<String> {
    let stable_id = instance_name(info);
    let host_name = format!("{stable_id}.local.");

    let service = ServiceInfo::new(
        SERVICE_TYPE,
        &stable_id,
        &host_name,
        "",
        info.port,
        txt_properties(info),
    )
    .map_err(|e| AppError::new(format!("failed to build mDNS service info: {e}")))?
    .enable_addr_auto();

    let fullname = service.get_fullname().to_string();

    daemon
        .register(service)
        .map_err(|e| AppError::new(format!("failed to register mDNS service: {e}")))?;

    Ok(fullname)
}

#[cfg(not(target_os = "ios"))]
pub fn stop_advertising(daemon: &ServiceDaemon, fullname: &str) -> AppResult<()> {
    daemon
        .unregister(fullname)
        .map_err(|e| AppError::new(format!("failed to unregister mDNS service: {e}")))?;
    Ok(())
}


#[cfg(not(target_os = "ios"))]
pub fn browse(daemon: &ServiceDaemon) -> AppResult<mdns_sd::Receiver<ServiceEvent>> {
    daemon
        .browse(SERVICE_TYPE)
        .map_err(|e| AppError::new(format!("failed to browse for mDNS services: {e}")))
}

#[cfg(not(target_os = "ios"))]
pub fn stop_browsing(daemon: &ServiceDaemon) -> AppResult<()> {
    daemon
        .stop_browse(SERVICE_TYPE)
        .map_err(|e| AppError::new(format!("failed to stop mDNS browse: {e}")))
}


pub fn decode_parts(
    fullname: &str,
    get: impl Fn(&str) -> Option<String>,
    addresses: Vec<std::net::IpAddr>,
    port: u16,
) -> Option<NearbyDevice> {
    let hostname = get(TXT_HOSTNAME)?;
    let session_id = get(TXT_SESSION_ID)?;
    let fingerprint = get(TXT_FINGERPRINT)?;
    let protocol_version = get(TXT_PROTOCOL_VERSION).unwrap_or_else(|| "unknown".to_string());
    let os = get(TXT_OS).unwrap_or_else(|| "unknown".to_string());

    Some(NearbyDevice {
        fullname: fullname.to_string(),
        hostname,
        session_id,
        fingerprint,
        protocol_version,
        os,
        addresses,
        port,
    })
}

#[cfg(not(target_os = "ios"))]
pub fn decode_resolved(resolved: &mdns_sd::ResolvedService) -> Option<NearbyDevice> {
    decode_parts(
        &resolved.fullname,
        |key| {
            resolved
                .txt_properties
                .get_property_val_str(key)
                .map(str::to_string)
        },
        resolved.addresses.iter().map(|a| a.to_ip_addr()).collect(),
        resolved.port,
    )
}
