#[cfg(not(target_os = "ios"))]
use mdns_sd::ServiceDaemon;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::discovery::bridge;
use crate::discovery::events::NearbyDevicePayload;
use crate::discovery::service::{self, LocalDeviceInfo};
use crate::error::AppResult;
use crate::identity::DeviceIdentity;
use crate::net;
use crate::state::AppState;
use crate::transport::endpoint as transport_endpoint;

pub fn local_display_name() -> String {
    crate::platform::device_name()
}

pub fn effective_local_name(app: &AppHandle) -> String {
    app.try_state::<crate::profile::MyProfileStore>()
        .and_then(|s| s.display_name_override())
        .unwrap_or_else(local_display_name)
}

#[cfg(not(target_os = "ios"))]
fn shared_daemon(discovery: &mut crate::state::DiscoveryState) -> AppResult<ServiceDaemon> {
    if let Some(d) = &discovery.daemon {
        return Ok(d.clone());
    }
    let d = ServiceDaemon::new()
        .map_err(|e| crate::error::AppError::new(format!("mDNS daemon init failed: {e}")))?;
    discovery.daemon = Some(d.clone());
    Ok(d)
}

#[tauri::command]
pub async fn become_visible(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    {
        let discovery = state.discovery.lock().await;
        if discovery.own_fullname.is_some() {
            return Ok(());
        }
    }

    let endpoint = net::ensure_endpoint(&app).await?;
    let port = transport_endpoint::local_port(&endpoint)?;

    let mut discovery = state.discovery.lock().await;

    let identity = app.state::<DeviceIdentity>();
    let info = LocalDeviceInfo {
        display_name: effective_local_name(&app),
        session_id: Uuid::new_v4().to_string(),
        fingerprint: identity.fingerprint(),
        port,
    };

    #[cfg(target_os = "ios")]
    let fullname = {
        let name = service::instance_name(&info);
        crate::platform::bonjour::publish(&app, &name, info.port, &service::txt_properties(&info))?;
        format!("{name}.{}", service::SERVICE_TYPE)
    };
    #[cfg(not(target_os = "ios"))]
    let fullname = service::advertise(&shared_daemon(&mut discovery)?, &info)?;

    discovery.own_fullname = Some(fullname);

    Ok(())
}

#[tauri::command]
pub async fn stop_visible(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let mut discovery = state.discovery.lock().await;

    let Some(fullname) = discovery.own_fullname.take() else {
        return Ok(());
    };

    #[cfg(target_os = "ios")]
    {
        let _ = &fullname;
        crate::platform::bonjour::unpublish(&app)?;
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = &app;
        if let Some(daemon) = discovery.daemon.clone() {
            service::stop_advertising(&daemon, &fullname)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn start_browsing(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let mut discovery = state.discovery.lock().await;

    if discovery.browse_task.is_some() {
        return Ok(());
    }

    #[cfg(target_os = "ios")]
    {
        crate::platform::bonjour::browse(&app)?;
        discovery.browse_task = Some(bridge::spawn_bonjour_bridge(app));
    }
    #[cfg(not(target_os = "ios"))]
    {
        let receiver = service::browse(&shared_daemon(&mut discovery)?)?;
        discovery.browse_task = Some(bridge::spawn_bridge(app, receiver));
    }

    Ok(())
}

#[tauri::command]
pub async fn stop_browsing(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let mut discovery = state.discovery.lock().await;

    #[cfg(target_os = "ios")]
    crate::platform::bonjour::stop_browse(&app)?;
    #[cfg(not(target_os = "ios"))]
    {
        let _ = &app;
        if let Some(daemon) = discovery.daemon.clone() {
            service::stop_browsing(&daemon)?;
        }
    }
    if let Some(task) = discovery.browse_task.take() {
        task.abort();
    }
    discovery.nearby.clear();

    Ok(())
}

#[tauri::command]
pub fn get_local_device_name(app: AppHandle) -> String {
    effective_local_name(&app)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LocalConnectionInfo {
    pub device_name: String,


    pub addresses: Vec<String>,

    pub port: u16,

    pub fingerprint: String,
}


pub(crate) fn primary_lan_ipv4() -> Vec<String> {
    use std::net::UdpSocket;
    let mut out = Vec::new();
    if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = sock.local_addr() {
                out.push(addr.ip().to_string());
            }
        }
    }
    out
}


#[tauri::command]
pub async fn get_local_connection_info(app: AppHandle) -> AppResult<LocalConnectionInfo> {
    let endpoint = net::ensure_endpoint(&app).await?;
    let port = transport_endpoint::local_port(&endpoint)?;
    let fingerprint = app.state::<DeviceIdentity>().fingerprint();
    Ok(LocalConnectionInfo {
        device_name: effective_local_name(&app),
        addresses: primary_lan_ipv4(),
        port,
        fingerprint,
    })
}

#[tauri::command]
pub async fn list_nearby_devices(state: State<'_, AppState>) -> AppResult<Vec<NearbyDevicePayload>> {
    let discovery = state.discovery.lock().await;
    Ok(discovery
        .nearby
        .values()
        .map(NearbyDevicePayload::from)
        .collect())
}
