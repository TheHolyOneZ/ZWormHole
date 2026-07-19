

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::blocked::{BlockStore, BlockedDevice};
use crate::commands::discovery::effective_local_name;
use crate::error::{AppError, AppResult};
use crate::net;
use crate::state::AppState;
use crate::trusted::store::TrustedStore;


#[tauri::command]
pub async fn send_connect_request(
    app: AppHandle,
    fullname: String,
    expiry_secs: Option<u64>,
) -> AppResult<Option<String>> {
    let device = {
        let state = app.state::<AppState>();
        let discovery = state.discovery.lock().await;
        discovery
            .nearby
            .get(&fullname)
            .cloned()
            .ok_or_else(|| AppError::new("device is no longer visible on the network"))?
    };


    let addr = device
        .addresses
        .iter()
        .find(|a| a.is_ipv4())
        .or_else(|| device.addresses.first())
        .ok_or_else(|| AppError::new("device has no known address"))?;
    let target_addr = std::net::SocketAddr::new(*addr, device.port);

    let endpoint = net::ensure_endpoint(&app).await?;
    let my_name = effective_local_name(&app);

    net::send_connect_request(
        app,
        endpoint,
        target_addr,
        Some(device.fingerprint),
        Some(device.hostname),
        my_name,
        expiry_secs,
    )
    .await
}


fn parse_target_addr(input: &str) -> AppResult<std::net::SocketAddr> {
    let s = input.trim();
    if let Ok(addr) = s.parse::<std::net::SocketAddr>() {
        return Ok(addr);
    }
    if let Ok(ip) = s.parse::<std::net::IpAddr>() {
        return Ok(std::net::SocketAddr::new(
            ip,
            crate::transport::endpoint::DEFAULT_QUIC_PORT,
        ));
    }
    Err(AppError::new(
        "enter an IPv4 address, optionally with a port — e.g. 192.168.1.5 or 192.168.1.5:58821",
    ))
}


#[tauri::command]
pub async fn send_manual_connect_request(
    app: AppHandle,
    address: String,
    expiry_secs: Option<u64>,
) -> AppResult<Option<String>> {
    let target_addr = parse_target_addr(&address)?;
    let endpoint = net::ensure_endpoint(&app).await?;
    let my_name = effective_local_name(&app);
    net::send_connect_request(
        app,
        endpoint,
        target_addr,
        None,
        None,
        my_name,
        expiry_secs,
    )
    .await
}


#[tauri::command]
pub async fn confirm_pairing_code(app: AppHandle, request_id: String, accept: bool) -> AppResult<()> {
    net::confirm_inbound_pairing(app, request_id, accept).await
}

#[derive(Debug, Clone, Serialize)]
pub struct TrustedDevicePayload {
    pub fingerprint: String,
    pub display_name: String,
    pub added_at: String,
    pub expires_at: Option<u64>,
    pub nickname: Option<String>,
    pub favorite: bool,
    pub auto_accept: bool,
}

impl From<crate::trusted::store::TrustedDevice> for TrustedDevicePayload {
    fn from(d: crate::trusted::store::TrustedDevice) -> Self {
        Self {
            fingerprint: d.fingerprint,
            display_name: d.display_name,
            added_at: d.added_at,
            expires_at: d.expires_at,
            nickname: d.nickname,
            favorite: d.favorite,
            auto_accept: d.auto_accept,
        }
    }
}

#[tauri::command]
pub fn list_trusted_devices(store: State<'_, TrustedStore>) -> AppResult<Vec<TrustedDevicePayload>> {

    let mut devices: Vec<TrustedDevicePayload> =
        store.list()?.into_iter().map(Into::into).collect();
    devices.sort_by(|a, b| {
        b.favorite
            .cmp(&a.favorite)
            .then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
    });
    Ok(devices)
}


#[tauri::command]
pub fn set_device_nickname(
    store: State<'_, TrustedStore>,
    fingerprint: String,
    nickname: Option<String>,
) -> AppResult<()> {
    store.set_nickname(&fingerprint, nickname)
}


#[tauri::command]
pub fn set_device_favorite(
    store: State<'_, TrustedStore>,
    fingerprint: String,
    favorite: bool,
) -> AppResult<()> {
    store.set_favorite(&fingerprint, favorite)
}


#[tauri::command]
pub fn set_device_auto_accept(
    store: State<'_, TrustedStore>,
    fingerprint: String,
    auto_accept: bool,
) -> AppResult<()> {
    store.set_auto_accept(&fingerprint, auto_accept)
}


#[tauri::command]
pub async fn revoke_trusted_device(
    app: AppHandle,
    store: State<'_, TrustedStore>,
    fingerprint: String,
) -> AppResult<()> {
    store.revoke(&fingerprint)?;
    net::disconnect_peer(&app, &fingerprint).await;
    Ok(())
}


#[tauri::command]
pub async fn block_device(
    app: AppHandle,
    trusted: State<'_, TrustedStore>,
    blocked: State<'_, BlockStore>,
    fingerprint: String,
    display_name: String,
) -> AppResult<()> {
    blocked.block(BlockedDevice {
        fingerprint: fingerprint.clone(),
        display_name,
        blocked_at: format!("{}", crate::net::now_unix_secs()),
    })?;


    let _ = trusted.revoke(&fingerprint);
    net::disconnect_peer(&app, &fingerprint).await;
    Ok(())
}

#[tauri::command]
pub async fn unblock_device(
    blocked: State<'_, BlockStore>,
    fingerprint: String,
) -> AppResult<()> {
    blocked.unblock(&fingerprint)
}

#[tauri::command]
pub async fn list_blocked_devices(
    blocked: State<'_, BlockStore>,
) -> AppResult<Vec<BlockedDevice>> {
    blocked.list()
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivePeerPayload {
    pub device_key: String,
    pub display_name: String,


    pub remote_addr: String,
}


#[tauri::command]
pub async fn list_active_peers(state: State<'_, AppState>) -> AppResult<Vec<ActivePeerPayload>> {
    let net = state.net.lock().await;
    Ok(net
        .peers
        .iter()
        .map(|(device_key, peer)| ActivePeerPayload {
            device_key: device_key.clone(),
            display_name: peer.display_name.clone(),
            remote_addr: peer.connection.remote_address().to_string(),
        })
        .collect())
}
