use mdns_sd::{Receiver, ServiceEvent};
use tauri::{AppHandle, Emitter, Manager};
use tokio::task::JoinHandle;

use super::events::{
    DeviceLostPayload, NearbyDevicePayload, DEVICE_APPEARED, DEVICE_LOST, DEVICE_UPDATED,
};
use super::service::decode_resolved;
use crate::state::AppState;


pub fn spawn_bridge(app: AppHandle, receiver: Receiver<ServiceEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Ok(event) = receiver.recv_async().await {
            match event {
                ServiceEvent::ServiceResolved(resolved) => {
                    let Some(device) = decode_resolved(&resolved) else {
                        continue;
                    };

                    let state = app.state::<AppState>();
                    let mut discovery = state.discovery.lock().await;


                    if discovery.own_fullname.as_deref() == Some(device.fullname.as_str()) {
                        continue;
                    }


                    if app
                        .try_state::<crate::identity::DeviceIdentity>()
                        .is_some_and(|id| id.fingerprint() == device.fingerprint)
                    {
                        continue;
                    }


                    if app
                        .try_state::<crate::blocked::BlockStore>()
                        .is_some_and(|store| store.is_blocked(&device.fingerprint))
                    {
                        continue;
                    }

                    let is_new = !discovery.nearby.contains_key(&device.fullname);
                    discovery
                        .nearby
                        .insert(device.fullname.clone(), device.clone());
                    drop(discovery);

                    let payload = NearbyDevicePayload::from(&device);
                    let event_name = if is_new {
                        DEVICE_APPEARED
                    } else {
                        DEVICE_UPDATED
                    };
                    let _ = app.emit(event_name, payload);
                }
                ServiceEvent::ServiceRemoved(_ty_domain, fullname) => {
                    let state = app.state::<AppState>();
                    let mut discovery = state.discovery.lock().await;
                    discovery.nearby.remove(&fullname);
                    drop(discovery);

                    let _ = app.emit(DEVICE_LOST, DeviceLostPayload { fullname });
                }
                _ => {}
            }
        }
    })
}
