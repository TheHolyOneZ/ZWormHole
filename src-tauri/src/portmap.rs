

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use crate::error::{AppError, AppResult};


pub const LEASE_SECS: u32 = 7200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Upnp,
    NatPmp,
}

#[derive(Debug, Clone)]
pub struct WanMapping {
    pub external_ip: String,
    pub external_port: u16,
    pub internal_port: u16,
    pub protocol: Protocol,


    pub public: bool,
}


pub async fn add_mapping(internal_ip: Ipv4Addr, internal_port: u16) -> AppResult<WanMapping> {
    let upnp_err = match upnp_add(internal_ip, internal_port).await {
        Ok(m) => return Ok(m),
        Err(e) => e,
    };
    match natpmp_add(internal_port).await {
        Ok(m) => Ok(m),
        Err(natpmp_err) => Err(AppError::new(format!(
            "Couldn't reach your router for automatic port forwarding. Check that UPnP is \
             enabled in the router's settings, and that a local firewall isn't dropping the \
             reply (on Linux, allow UDP 1900/SSDP). Otherwise, forward UDP port {internal_port} \
             to this machine manually. (UPnP: {upnp_err} · NAT-PMP: {natpmp_err})"
        ))),
    }
}


fn search_opts(bind_ip: Option<Ipv4Addr>) -> igd_next::SearchOptions {
    let mut opts = igd_next::SearchOptions {
        timeout: Some(Duration::from_secs(6)),
        ..Default::default()
    };
    if let Some(ip) = bind_ip {
        opts.bind_addr = SocketAddr::new(IpAddr::V4(ip), 0);
    }
    opts
}


pub async fn remove_mapping(m: &WanMapping) -> AppResult<()> {
    match m.protocol {
        Protocol::Upnp => upnp_remove(m.external_port).await,
        Protocol::NatPmp => natpmp_remove(m.internal_port).await,
    }
}


pub fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    let cgnat = o[0] == 100 && (64..=127).contains(&o[1]);
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || cgnat
        || o[0] >= 224)
}


async fn upnp_add(internal_ip: Ipv4Addr, internal_port: u16) -> AppResult<WanMapping> {
    use igd_next::aio::tokio::search_gateway;
    use igd_next::PortMappingProtocol;

    let gw = search_gateway(search_opts(Some(internal_ip)))
        .await
        .map_err(|e| AppError::new(format!("no UPnP gateway found: {e}")))?;
    let external_ip = gw
        .get_external_ip()
        .await
        .map_err(|e| AppError::new(format!("couldn't read external IP: {e}")))?;
    let local = SocketAddr::new(IpAddr::V4(internal_ip), internal_port);
    let external_port = gw
        .add_any_port(PortMappingProtocol::UDP, local, LEASE_SECS, "ZWormHole")
        .await
        .map_err(|e| AppError::new(format!("couldn't add port mapping: {e}")))?;

    let public = matches!(external_ip, IpAddr::V4(v4) if is_public_ipv4(v4));
    Ok(WanMapping {
        external_ip: external_ip.to_string(),
        external_port,
        internal_port,
        protocol: Protocol::Upnp,
        public,
    })
}

async fn upnp_remove(external_port: u16) -> AppResult<()> {
    use igd_next::aio::tokio::search_gateway;
    use igd_next::PortMappingProtocol;

    let gw = search_gateway(search_opts(None))
        .await
        .map_err(|e| AppError::new(format!("no UPnP gateway: {e}")))?;
    gw.remove_port(PortMappingProtocol::UDP, external_port)
        .await
        .map_err(|e| AppError::new(format!("couldn't remove mapping: {e}")))
}


async fn natpmp_add(internal_port: u16) -> AppResult<WanMapping> {
    tokio::task::spawn_blocking(move || natpmp_add_blocking(internal_port))
        .await
        .map_err(|e| AppError::new(format!("NAT-PMP task failed: {e}")))?
}

async fn natpmp_remove(internal_port: u16) -> AppResult<()> {
    tokio::task::spawn_blocking(move || natpmp_remove_blocking(internal_port))
        .await
        .map_err(|e| AppError::new(format!("NAT-PMP task failed: {e}")))?
}

fn natpmp_read<T>(
    n: &mut natpmp::Natpmp,
    pick: impl Fn(natpmp::Response) -> Option<T>,
) -> AppResult<T> {

    for _ in 0..20 {
        match n.read_response_or_retry() {
            Ok(resp) => {
                if let Some(v) = pick(resp) {
                    return Ok(v);
                }
                return Err(AppError::new("unexpected NAT-PMP response"));
            }
            Err(natpmp::Error::NATPMP_TRYAGAIN) => {
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            Err(e) => return Err(AppError::new(format!("NAT-PMP error: {e:?}"))),
        }
    }
    Err(AppError::new("NAT-PMP gateway did not respond"))
}

fn guessed_gateway() -> Option<Ipv4Addr> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("192.0.2.1:9").ok()?;
    match sock.local_addr().ok()?.ip() {
        IpAddr::V4(v4) if v4.is_private() => {
            let [a, b, c, _] = v4.octets();
            Some(Ipv4Addr::new(a, b, c, 1))
        }
        _ => None,
    }
}

fn natpmp_client() -> AppResult<natpmp::Natpmp> {
    match natpmp::Natpmp::new() {
        Ok(n) => Ok(n),
        Err(e) => guessed_gateway()
            .and_then(|gw| natpmp::Natpmp::new_with(gw).ok())
            .ok_or_else(|| AppError::new(format!("NAT-PMP init: {e:?}"))),
    }
}

fn natpmp_add_blocking(internal_port: u16) -> AppResult<WanMapping> {
    use natpmp::{Protocol as NatProto, Response};

    let mut n = natpmp_client()?;

    n.send_public_address_request()
        .map_err(|e| AppError::new(format!("NAT-PMP address request: {e:?}")))?;
    let external_ip = natpmp_read(&mut n, |r| match r {
        Response::Gateway(g) => Some(*g.public_address()),
        _ => None,
    })?;

    n.send_port_mapping_request(NatProto::UDP, internal_port, internal_port, LEASE_SECS)
        .map_err(|e| AppError::new(format!("NAT-PMP map request: {e:?}")))?;
    let external_port = natpmp_read(&mut n, |r| match r {
        Response::UDP(m) => Some(m.public_port()),
        _ => None,
    })?;

    Ok(WanMapping {
        external_ip: external_ip.to_string(),
        external_port,
        internal_port,
        protocol: Protocol::NatPmp,
        public: is_public_ipv4(external_ip),
    })
}

fn natpmp_remove_blocking(internal_port: u16) -> AppResult<()> {
    use natpmp::Protocol as NatProto;

    let mut n = natpmp_client()?;
    n.send_port_mapping_request(NatProto::UDP, internal_port, internal_port, 0)
        .map_err(|e| AppError::new(format!("NAT-PMP unmap: {e:?}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_private_and_cgnat_as_not_public() {

        assert!(!is_public_ipv4(Ipv4Addr::new(192, 168, 1, 10)));
        assert!(!is_public_ipv4(Ipv4Addr::new(10, 0, 0, 5)));
        assert!(!is_public_ipv4(Ipv4Addr::new(172, 16, 4, 2)));

        assert!(!is_public_ipv4(Ipv4Addr::new(100, 64, 0, 1)));
        assert!(!is_public_ipv4(Ipv4Addr::new(100, 127, 255, 254)));

        assert!(!is_public_ipv4(Ipv4Addr::new(127, 0, 0, 1)));
        assert!(!is_public_ipv4(Ipv4Addr::new(169, 254, 1, 1)));
    }

    #[test]
    fn accepts_real_public_addresses() {
        assert!(is_public_ipv4(Ipv4Addr::new(203, 0, 113, 7)));
        assert!(is_public_ipv4(Ipv4Addr::new(8, 8, 8, 8)));

        assert!(is_public_ipv4(Ipv4Addr::new(100, 0, 0, 1)));
    }
}
