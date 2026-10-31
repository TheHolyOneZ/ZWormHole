#[cfg(not(target_os = "ios"))]
use mdns_sd::{Receiver, ServiceEvent};
use tauri::{AppHandle, Emitter, Manager};
use tokio::task::JoinHandle;

use super::events::{
    DeviceLostPayload, NearbyDevicePayload, DEVICE_APPEARED, DEVICE_LOST, DEVICE_UPDATED,
};
#[cfg(not(target_os = "ios"))]
use super::service::decode_resolved;
use super::service::NearbyDevice;
use crate::state::AppState;

#[cfg(not(target_os = "ios"))]
pub fn spawn_bridge(app: AppHandle, receiver: Receiver<ServiceEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Ok(event) = receiver.recv_async().await {
            match event {
                ServiceEvent::ServiceResolved(resolved) => {
                    if let Some(device) = decode_resolved(&resolved) {
                        device_resolved(&app, device).await;
                    }
                }
                ServiceEvent::ServiceRemoved(_ty_domain, fullname) => {
                    device_removed(&app, fullname).await;
                }
                _ => {}
            }
        }
    })
}

async fn device_resolved(app: &AppHandle, device: NearbyDevice) {
    let state = app.state::<AppState>();
    let mut discovery = state.discovery.lock().await;

    if discovery.own_fullname.as_deref() == Some(device.fullname.as_str()) {
        return;
    }

    if app
        .try_state::<crate::identity::DeviceIdentity>()
        .is_some_and(|id| id.fingerprint() == device.fingerprint)
    {
        return;
    }

    if app
        .try_state::<crate::blocked::BlockStore>()
        .is_some_and(|store| store.is_blocked(&device.fingerprint))
    {
        return;
    }

    let previous = discovery
        .nearby
        .insert(device.fullname.clone(), device.clone());
    drop(discovery);

    let event_name = match previous {
        None => DEVICE_APPEARED,
        Some(prev) if same_device(&prev, &device) => return,
        Some(_) => DEVICE_UPDATED,
    };
    let _ = app.emit(event_name, NearbyDevicePayload::from(&device));
}

fn same_device(a: &NearbyDevice, b: &NearbyDevice) -> bool {
    a.hostname == b.hostname
        && a.session_id == b.session_id
        && a.fingerprint == b.fingerprint
        && a.protocol_version == b.protocol_version
        && a.os == b.os
        && a.addresses == b.addresses
        && a.port == b.port
}

async fn device_removed(app: &AppHandle, fullname: String) {
    let state = app.state::<AppState>();
    let mut discovery = state.discovery.lock().await;
    let existed = discovery.nearby.remove(&fullname).is_some();
    drop(discovery);

    if existed {
        let _ = app.emit(DEVICE_LOST, DeviceLostPayload { fullname });
    }
}

#[cfg(target_os = "ios")]
pub fn spawn_bonjour_bridge(app: AppHandle) -> JoinHandle<()> {
    use std::collections::HashSet;

    tokio::spawn(async move {
        let mut known: HashSet<String> = HashSet::new();
        loop {
            let snapshot = {
                let app = app.clone();
                tokio::task::spawn_blocking(move || crate::platform::bonjour::snapshot(&app))
                    .await
                    .ok()
                    .and_then(Result::ok)
            };
            if let Some(services) = snapshot {
                let mut seen: HashSet<String> = HashSet::new();
                for service in services {
                    let addresses: Vec<std::net::IpAddr> = service
                        .addresses
                        .iter()
                        .filter_map(|a| a.parse().ok())
                        .collect();
                    if addresses.is_empty() {
                        continue;
                    }
                    let txt = service.txt.clone();
                    let Some(device) = super::service::decode_parts(
                        &service.fullname,
                        |key| txt.get(key).cloned(),
                        addresses,
                        service.port,
                    ) else {
                        continue;
                    };
                    seen.insert(device.fullname.clone());
                    device_resolved(&app, device).await;
                }
                for gone in known.difference(&seen) {
                    device_removed(&app, gone.clone()).await;
                }
                known = seen;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        }
    })
}
