

use std::net::Ipv4Addr;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::portmap::{self, Protocol, WanMapping, LEASE_SECS};
use crate::state::AppState;
use crate::transport::endpoint as transport_endpoint;

#[derive(Serialize, Clone)]
pub struct WanStatusPayload {
    pub external_ip: String,
    pub external_port: u16,

    pub protocol: String,


    pub public: bool,
}

impl From<&WanMapping> for WanStatusPayload {
    fn from(m: &WanMapping) -> Self {
        Self {
            external_ip: m.external_ip.clone(),
            external_port: m.external_port,
            protocol: match m.protocol {
                Protocol::Upnp => "UPnP".to_string(),
                Protocol::NatPmp => "NAT-PMP".to_string(),
            },
            public: m.public,
        }
    }
}

fn local_lan_ipv4() -> AppResult<Ipv4Addr> {
    crate::commands::discovery::primary_lan_ipv4()
        .into_iter()
        .find_map(|a| a.parse::<Ipv4Addr>().ok())
        .ok_or_else(|| AppError::new("couldn't determine this machine's LAN IPv4 address"))
}


#[tauri::command]
pub async fn enable_internet_reachable(app: AppHandle) -> AppResult<WanStatusPayload> {
    let endpoint = crate::net::ensure_endpoint(&app).await?;
    let internal_port = transport_endpoint::local_port(&endpoint)?;
    let internal_ip = local_lan_ipv4()?;

    let mapping = portmap::add_mapping(internal_ip, internal_port).await?;
    let payload = WanStatusPayload::from(&mapping);

    let generation = {
        let state = app.state::<AppState>();
        let mut wan = state.wan.lock().await;
        wan.generation = wan.generation.wrapping_add(1);
        wan.mapping = Some(mapping);
        wan.generation
    };
    spawn_lease_refresh(app.clone(), generation, internal_port);
    Ok(payload)
}


#[tauri::command]
pub async fn disable_internet_reachable(app: AppHandle) -> AppResult<()> {
    let mapping = {
        let state = app.state::<AppState>();
        let mut wan = state.wan.lock().await;
        wan.generation = wan.generation.wrapping_add(1);
        wan.mapping.take()
    };
    if let Some(m) = mapping {
        let _ = portmap::remove_mapping(&m).await;
    }
    Ok(())
}


#[tauri::command]
pub async fn get_wan_status(app: AppHandle) -> AppResult<Option<WanStatusPayload>> {
    let state = app.state::<AppState>();
    let wan = state.wan.lock().await;
    Ok(wan.mapping.as_ref().map(WanStatusPayload::from))
}


pub async fn release_on_exit(app: &AppHandle) {
    let mapping = {
        let state = app.state::<AppState>();
        let mut wan = state.wan.lock().await;
        wan.mapping.take()
    };
    if let Some(m) = mapping {
        let _ = portmap::remove_mapping(&m).await;
    }
}


fn spawn_lease_refresh(app: AppHandle, generation: u64, internal_port: u16) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs((LEASE_SECS / 2) as u64)).await;
            {
                let state = app.state::<AppState>();
                let wan = state.wan.lock().await;
                if wan.generation != generation {
                    break;
                }
            }
            let internal_ip = match local_lan_ipv4() {
                Ok(ip) => ip,
                Err(_) => continue,
            };
            if let Ok(m) = portmap::add_mapping(internal_ip, internal_port).await {
                let state = app.state::<AppState>();
                let mut wan = state.wan.lock().await;
                if wan.generation == generation {
                    wan.mapping = Some(m);
                } else {
                    break;
                }
            }
        }
    });
}
