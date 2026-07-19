use std::net::SocketAddr;
use std::sync::{Arc, Once};
use std::time::Duration;

use quinn::crypto::rustls::QuicClientConfig;
use quinn::rustls;
use quinn::{ClientConfig, Endpoint, ServerConfig, TransportConfig, VarInt};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};

use crate::error::{AppError, AppResult};

static INSTALL_CRYPTO_PROVIDER: Once = Once::new();

fn ensure_crypto_provider_installed() {
    INSTALL_CRYPTO_PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}


fn generate_self_signed_cert() -> AppResult<(CertificateDer<'static>, PrivateKeyDer<'static>)> {
    let cert = rcgen::generate_simple_self_signed(vec!["zwormhole.local".to_string()])
        .map_err(|e| AppError::new(format!("failed to generate ephemeral TLS cert: {e}")))?;
    let cert_der = cert.cert.der().clone();
    let key_der = PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
    Ok((cert_der, PrivateKeyDer::Pkcs8(key_der)))
}


#[derive(Debug)]
struct AcceptAnyServerCert;

impl ServerCertVerifier for AcceptAnyServerCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}


fn build_transport_config() -> Arc<TransportConfig> {
    let mut config = TransportConfig::default();
    config.max_idle_timeout(Some(Duration::from_secs(40).try_into().unwrap()));
    config.keep_alive_interval(Some(Duration::from_secs(10)));


    config.stream_receive_window(VarInt::from_u32(16 * 1024 * 1024));
    config.receive_window(VarInt::from_u32(32 * 1024 * 1024));
    config.send_window(32 * 1024 * 1024);
    Arc::new(config)
}

fn build_server_config() -> AppResult<ServerConfig> {
    let (cert, key) = generate_self_signed_cert()?;
    let mut server_config = ServerConfig::with_single_cert(vec![cert], key)
        .map_err(|e| AppError::new(format!("failed to build QUIC server config: {e}")))?;
    server_config.transport_config(build_transport_config());
    Ok(server_config)
}

fn build_client_config() -> AppResult<ClientConfig> {
    let rustls_config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyServerCert))
        .with_no_client_auth();

    let quic_client_config = QuicClientConfig::try_from(rustls_config)
        .map_err(|e| AppError::new(format!("failed to build QUIC client config: {e}")))?;

    let mut client_config = ClientConfig::new(Arc::new(quic_client_config));
    client_config.transport_config(build_transport_config());
    Ok(client_config)
}


pub const DEFAULT_QUIC_PORT: u16 = 58821;


pub fn bind(port: u16) -> AppResult<Endpoint> {
    ensure_crypto_provider_installed();

    let server_config = build_server_config()?;
    let client_config = build_client_config()?;

    let addr: SocketAddr = format!("0.0.0.0:{port}")
        .parse()
        .map_err(|e| AppError::new(format!("invalid bind address: {e}")))?;

    let mut endpoint = Endpoint::server(server_config, addr).map_err(|e| describe_bind_error(e, port))?;
    endpoint.set_default_client_config(client_config);

    Ok(endpoint)
}


pub fn bind_default() -> AppResult<Endpoint> {
    match bind(DEFAULT_QUIC_PORT) {
        Ok(endpoint) => Ok(endpoint),
        Err(_) => bind(0),
    }
}


fn describe_bind_error(e: std::io::Error, port: u16) -> AppError {
    use std::io::ErrorKind;
    let hint = match e.kind() {
        ErrorKind::AddrInUse => {
            "another process is already using this port".to_string()
        }
        ErrorKind::PermissionDenied => {
            "permission denied — on Windows this usually means the firewall prompt was denied; on Linux, binding privileged ports needs elevated permissions".to_string()
        }
        _ => e.to_string(),
    };
    AppError::new(format!("failed to bind QUIC endpoint on port {port}: {hint}"))
}


pub fn local_port(endpoint: &Endpoint) -> AppResult<u16> {
    endpoint
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|e| AppError::new(format!("failed to read local QUIC address: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn binds_an_ephemeral_port() {
        let endpoint = bind(0).expect("bind should succeed on an ephemeral port");
        let port = local_port(&endpoint).expect("endpoint should report its local port");
        assert!(port > 0);
        endpoint.close(0u32.into(), b"test done");
    }

    #[tokio::test]
    async fn falls_back_to_ephemeral_when_default_port_is_taken() {
        let holder = bind(DEFAULT_QUIC_PORT).expect("first bind of the default port should succeed");


        let endpoint = bind_default().expect("bind_default should fall back, not fail");
        let port = local_port(&endpoint).expect("endpoint should report its local port");
        assert_ne!(port, DEFAULT_QUIC_PORT);

        holder.close(0u32.into(), b"test done");
        endpoint.close(0u32.into(), b"test done");
    }
}
