use std::time::Duration;

use quinn::{Connection, RecvStream, SendStream};
use rand::Rng;
use snow::{Builder, TransportState};

use crate::error::{AppError, AppResult};
use crate::identity::IK_PATTERN;


const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);


const NOISE_PATTERN: &str = "Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s";


const NOISE_OVERHEAD: usize = 128;


const MAX_NOISE_PLAINTEXT: usize = 65000;


pub struct NoiseSession {
    transport: TransportState,
    send: SendStream,
    recv: RecvStream,
}

async fn write_frame(send: &mut SendStream, bytes: &[u8]) -> AppResult<()> {


    let len = u16::try_from(bytes.len())
        .map_err(|_| AppError::new("noise frame exceeds 65535 bytes"))?;


    let mut framed = Vec::with_capacity(2 + bytes.len());
    framed.extend_from_slice(&len.to_be_bytes());
    framed.extend_from_slice(bytes);
    send.write_all(&framed)
        .await
        .map_err(|e| AppError::new(format!("failed to write noise frame: {e}")))
}

async fn read_frame(recv: &mut RecvStream) -> AppResult<Vec<u8>> {
    let mut len_buf = [0u8; 2];
    recv.read_exact(&mut len_buf)
        .await
        .map_err(|e| AppError::new(format!("failed to read noise frame length: {e}")))?;
    let len = u16::from_be_bytes(len_buf) as usize;

    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf)
        .await
        .map_err(|e| AppError::new(format!("failed to read noise frame: {e}")))?;
    Ok(buf)
}


pub async fn upgrade_outbound(connection: &Connection, psk: &[u8; 32]) -> AppResult<NoiseSession> {
    tokio::time::timeout(HANDSHAKE_TIMEOUT, upgrade_outbound_inner(connection, psk))
        .await
        .map_err(|_| {
            AppError::new(
                "timed out establishing a secure channel — the device may be offline",
            )
        })?
}

async fn upgrade_outbound_inner(connection: &Connection, psk: &[u8; 32]) -> AppResult<NoiseSession> {
    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .map_err(|e| AppError::new(format!("failed to open QUIC stream: {e}")))?;

    let mut handshake = Builder::new(
        NOISE_PATTERN
            .parse()
            .map_err(|e| AppError::new(format!("invalid noise pattern: {e}")))?,
    )
    .psk(0, psk)
    .map_err(|e| AppError::new(format!("failed to set noise psk: {e}")))?
    .build_initiator()
    .map_err(|e| AppError::new(format!("failed to build noise initiator: {e}")))?;


    let mut out = vec![0u8; NOISE_OVERHEAD];
    let len = handshake
        .write_message(&[], &mut out)
        .map_err(|e| AppError::new(format!("noise handshake write failed: {e}")))?;
    write_frame(&mut send, &out[..len]).await?;


    let inbound = read_frame(&mut recv).await?;
    let mut payload = vec![0u8; inbound.len()];
    handshake
        .read_message(&inbound, &mut payload)
        .map_err(|e| AppError::new(format!("noise handshake read failed: {e}")))?;

    let transport = handshake
        .into_transport_mode()
        .map_err(|e| AppError::new(format!("failed to enter noise transport mode: {e}")))?;

    Ok(NoiseSession {
        transport,
        send,
        recv,
    })
}


pub async fn upgrade_inbound_stream(
    mut send: SendStream,
    mut recv: RecvStream,
    psk: &[u8; 32],
) -> AppResult<NoiseSession> {
    let mut handshake = Builder::new(
        NOISE_PATTERN
            .parse()
            .map_err(|e| AppError::new(format!("invalid noise pattern: {e}")))?,
    )
    .psk(0, psk)
    .map_err(|e| AppError::new(format!("failed to set noise psk: {e}")))?
    .build_responder()
    .map_err(|e| AppError::new(format!("failed to build noise responder: {e}")))?;


    let inbound = read_frame(&mut recv).await?;
    let mut payload = vec![0u8; inbound.len()];
    handshake
        .read_message(&inbound, &mut payload)
        .map_err(|e| AppError::new(format!("noise handshake read failed: {e}")))?;


    let mut out = vec![0u8; NOISE_OVERHEAD];
    let len = handshake
        .write_message(&[], &mut out)
        .map_err(|e| AppError::new(format!("noise handshake write failed: {e}")))?;
    write_frame(&mut send, &out[..len]).await?;

    let transport = handshake
        .into_transport_mode()
        .map_err(|e| AppError::new(format!("failed to enter noise transport mode: {e}")))?;

    Ok(NoiseSession {
        transport,
        send,
        recv,
    })
}

impl NoiseSession {


    pub fn seal(&mut self, plaintext: &[u8]) -> AppResult<Vec<u8>> {
        let mut out = vec![0u8; 2 + plaintext.len() + NOISE_OVERHEAD];
        let len = self
            .transport
            .write_message(plaintext, &mut out[2..])
            .map_err(|e| AppError::new(format!("noise encrypt failed: {e}")))?;
        let framed_len =
            u16::try_from(len).map_err(|_| AppError::new("noise frame exceeds 65535 bytes"))?;
        out[0..2].copy_from_slice(&framed_len.to_be_bytes());
        out.truncate(2 + len);
        Ok(out)
    }


    pub async fn write_sealed(&mut self, framed: &[u8]) -> AppResult<()> {
        self.send
            .write_all(framed)
            .await
            .map_err(|e| AppError::new(format!("failed to write noise frame: {e}")))
    }


    pub async fn send(&mut self, plaintext: &[u8]) -> AppResult<()> {
        let mut msg = Vec::with_capacity(4 + plaintext.len());
        msg.extend_from_slice(&(plaintext.len() as u32).to_be_bytes());
        msg.extend_from_slice(plaintext);
        for piece in msg.chunks(MAX_NOISE_PLAINTEXT) {
            let framed = self.seal(piece)?;
            self.write_sealed(&framed).await?;
        }
        Ok(())
    }


    async fn recv_frame(&mut self) -> AppResult<Vec<u8>> {
        let inbound = read_frame(&mut self.recv).await?;
        let mut out = vec![0u8; inbound.len()];
        let len = self
            .transport
            .read_message(&inbound, &mut out)
            .map_err(|e| AppError::new(format!("noise decrypt failed: {e}")))?;
        out.truncate(len);
        Ok(out)
    }


    pub async fn recv(&mut self) -> AppResult<Vec<u8>> {
        let mut buf = self.recv_frame().await?;
        while buf.len() < 4 {
            let more = self.recv_frame().await?;
            buf.extend_from_slice(&more);
        }
        let total = u32::from_be_bytes(buf[0..4].try_into().unwrap()) as usize;
        while buf.len() < 4 + total {
            let more = self.recv_frame().await?;
            buf.extend_from_slice(&more);
        }
        Ok(buf[4..4 + total].to_vec())
    }
}


pub async fn ik_derive_shared_secret_initiator(
    connection: &Connection,
    my_static: &[u8],
    peer_static_pub: &[u8],
) -> AppResult<[u8; 32]> {
    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .map_err(|e| AppError::new(format!("failed to open QUIC stream: {e}")))?;

    let mut handshake = Builder::new(
        IK_PATTERN
            .parse()
            .map_err(|e| AppError::new(format!("invalid noise pattern: {e}")))?,
    )
    .local_private_key(my_static)
    .map_err(|e| AppError::new(format!("failed to set local static key: {e}")))?
    .remote_public_key(peer_static_pub)
    .map_err(|e| AppError::new(format!("failed to set remote static key: {e}")))?
    .build_initiator()
    .map_err(|e| AppError::new(format!("failed to build noise IK initiator: {e}")))?;


    let mut out = vec![0u8; NOISE_OVERHEAD];
    let len = handshake
        .write_message(&[], &mut out)
        .map_err(|e| AppError::new(format!("IK handshake write failed: {e}")))?;
    write_frame(&mut send, &out[..len]).await?;


    let inbound = read_frame(&mut recv).await?;
    let mut payload = vec![0u8; inbound.len()];
    handshake
        .read_message(&inbound, &mut payload)
        .map_err(|e| AppError::new(format!("IK handshake read failed — peer's stored key may be stale: {e}")))?;

    let mut transport = handshake
        .into_transport_mode()
        .map_err(|e| AppError::new(format!("failed to enter noise transport mode: {e}")))?;

    let mut secret = [0u8; 32];
    rand::rng().fill_bytes(&mut secret);

    let mut out = vec![0u8; secret.len() + NOISE_OVERHEAD];
    let len = transport
        .write_message(&secret, &mut out)
        .map_err(|e| AppError::new(format!("failed to deliver session secret: {e}")))?;
    write_frame(&mut send, &out[..len]).await?;

    Ok(secret)
}


pub async fn ik_derive_shared_secret_responder(
    connection: &Connection,
    my_static: &[u8],
    peer_static_pub: &[u8],
) -> AppResult<[u8; 32]> {
    let (mut send, mut recv) = connection
        .accept_bi()
        .await
        .map_err(|e| AppError::new(format!("failed to accept QUIC stream: {e}")))?;

    let mut handshake = Builder::new(
        IK_PATTERN
            .parse()
            .map_err(|e| AppError::new(format!("invalid noise pattern: {e}")))?,
    )
    .local_private_key(my_static)
    .map_err(|e| AppError::new(format!("failed to set local static key: {e}")))?
    .remote_public_key(peer_static_pub)
    .map_err(|e| AppError::new(format!("failed to set remote static key: {e}")))?
    .build_responder()
    .map_err(|e| AppError::new(format!("failed to build noise IK responder: {e}")))?;


    let inbound = read_frame(&mut recv).await?;
    let mut payload = vec![0u8; inbound.len()];
    handshake
        .read_message(&inbound, &mut payload)
        .map_err(|e| AppError::new(format!("IK handshake read failed — peer's identity doesn't match what we have on file: {e}")))?;


    let mut out = vec![0u8; NOISE_OVERHEAD];
    let len = handshake
        .write_message(&[], &mut out)
        .map_err(|e| AppError::new(format!("IK handshake write failed: {e}")))?;
    write_frame(&mut send, &out[..len]).await?;

    let mut transport = handshake
        .into_transport_mode()
        .map_err(|e| AppError::new(format!("failed to enter noise transport mode: {e}")))?;

    let inbound = read_frame(&mut recv).await?;
    let mut out = vec![0u8; inbound.len()];
    let len = transport
        .read_message(&inbound, &mut out)
        .map_err(|e| AppError::new(format!("failed to receive session secret: {e}")))?;
    if len != 32 {
        return Err(AppError::new("session secret had unexpected length"));
    }
    let mut secret = [0u8; 32];
    secret.copy_from_slice(&out[..32]);
    Ok(secret)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::endpoint;


    #[tokio::test]
    async fn noise_over_quic_round_trip() {
        let server_ep = endpoint::bind(0).expect("server bind");
        let server_port = server_ep.local_addr().expect("server addr").port();
        let server_addr: std::net::SocketAddr = ([127, 0, 0, 1], server_port).into();

        let client_ep = endpoint::bind(0).expect("client bind");

        let psk = [7u8; 32];

        let server_task = tokio::spawn({
            let psk = psk;
            async move {
                let incoming = server_ep
                    .accept()
                    .await
                    .expect("no incoming connection");
                let connection = incoming.await.expect("server handshake failed");
                let (send, recv) = connection.accept_bi().await.expect("accept stream failed");
                let mut session = upgrade_inbound_stream(send, recv, &psk)
                    .await
                    .expect("server noise upgrade failed");

                let received = session.recv().await.expect("server recv failed");
                assert_eq!(received, b"hello from client");

                session
                    .send(b"hello from server")
                    .await
                    .expect("server send failed");


                let _ = session.recv().await;
                connection
            }
        });

        let connection = client_ep
            .connect(server_addr, "zwormhole.local")
            .expect("client connect setup failed")
            .await
            .expect("client handshake failed");

        let mut session = upgrade_outbound(&connection, &psk)
            .await
            .expect("client noise upgrade failed");

        session
            .send(b"hello from client")
            .await
            .expect("client send failed");

        let received = session.recv().await.expect("client recv failed");
        assert_eq!(received, b"hello from server");

        session
            .send(b"ack")
            .await
            .expect("client ack send failed");

        server_task.await.expect("server task panicked");
    }

    #[tokio::test]
    async fn large_message_spans_multiple_frames() {


        let server_ep = endpoint::bind(0).expect("server bind");
        let server_port = server_ep.local_addr().expect("server addr").port();
        let server_addr: std::net::SocketAddr = ([127, 0, 0, 1], server_port).into();
        let client_ep = endpoint::bind(0).expect("client bind");
        let psk = [9u8; 32];

        let payload: Vec<u8> = (0..250_000).map(|i| (i % 251) as u8).collect();
        let expected = payload.clone();

        let server_task = tokio::spawn({
            let psk = psk;
            async move {
                let incoming = server_ep.accept().await.expect("no incoming");
                let connection = incoming.await.expect("server handshake");
                let (send, recv) = connection.accept_bi().await.expect("accept stream");
                let mut session = upgrade_inbound_stream(send, recv, &psk)
                    .await
                    .expect("server upgrade");
                let received = session.recv().await.expect("server recv");
                assert_eq!(received.len(), expected.len(), "length mismatch");
                assert_eq!(received, expected, "payload corrupted across frames");
                session.send(b"ok").await.expect("server ack");
                connection
            }
        });

        let connection = client_ep
            .connect(server_addr, "zwormhole.local")
            .expect("client connect")
            .await
            .expect("client handshake");
        let mut session = upgrade_outbound(&connection, &psk)
            .await
            .expect("client upgrade");
        session.send(&payload).await.expect("client send large");
        let ack = session.recv().await.expect("client recv ack");
        assert_eq!(ack, b"ok");
        server_task.await.expect("server task panicked");
    }


    #[tokio::test]
    async fn mismatched_psk_fails_the_handshake() {
        let server_ep = endpoint::bind(0).expect("server bind");
        let server_port = server_ep.local_addr().expect("server addr").port();
        let server_addr: std::net::SocketAddr = ([127, 0, 0, 1], server_port).into();
        let client_ep = endpoint::bind(0).expect("client bind");

        let server_task = tokio::spawn(async move {
            let incoming = server_ep.accept().await.expect("no incoming connection");
            let connection = incoming.await.expect("server handshake failed");
            let (send, recv) = connection.accept_bi().await.expect("accept stream failed");
            let result = upgrade_inbound_stream(send, recv, &[1u8; 32]).await;
            assert!(result.is_err(), "handshake should fail on PSK mismatch");
        });

        let connection = client_ep
            .connect(server_addr, "zwormhole.local")
            .expect("client connect setup failed")
            .await
            .expect("client handshake failed");


        let _ = upgrade_outbound(&connection, &[2u8; 32]).await;

        server_task.await.expect("server task panicked");
    }

    fn generate_static_keypair() -> (Vec<u8>, Vec<u8>) {
        let params: snow::params::NoiseParams = IK_PATTERN.parse().unwrap();
        let keypair = Builder::new(params).generate_keypair().unwrap();
        (keypair.private, keypair.public)
    }


    #[tokio::test]
    async fn ik_fast_reconnect_derives_matching_secret() {
        let server_ep = endpoint::bind(0).expect("server bind");
        let server_port = server_ep.local_addr().expect("server addr").port();
        let server_addr: std::net::SocketAddr = ([127, 0, 0, 1], server_port).into();
        let client_ep = endpoint::bind(0).expect("client bind");

        let (initiator_priv, initiator_pub) = generate_static_keypair();
        let (responder_priv, responder_pub) = generate_static_keypair();

        let responder_priv_for_task = responder_priv.clone();
        let initiator_pub_for_task = initiator_pub.clone();
        let server_task = tokio::spawn(async move {
            let incoming = server_ep.accept().await.expect("no incoming connection");
            let connection = incoming.await.expect("server handshake failed");
            let secret = ik_derive_shared_secret_responder(
                &connection,
                &responder_priv_for_task,
                &initiator_pub_for_task,
            )
            .await
            .expect("responder IK handshake failed");
            (secret, connection)
        });

        let connection = client_ep
            .connect(server_addr, "zwormhole.local")
            .expect("client connect setup failed")
            .await
            .expect("client handshake failed");

        let client_secret =
            ik_derive_shared_secret_initiator(&connection, &initiator_priv, &responder_pub)
                .await
                .expect("initiator IK handshake failed");

        let (server_secret, _connection) = server_task.await.expect("server task panicked");

        assert_eq!(client_secret, server_secret);


        assert_ne!(client_secret, [0u8; 32]);
    }


    #[tokio::test]
    async fn ik_fails_when_stored_key_does_not_match() {
        let server_ep = endpoint::bind(0).expect("server bind");
        let server_port = server_ep.local_addr().expect("server addr").port();
        let server_addr: std::net::SocketAddr = ([127, 0, 0, 1], server_port).into();
        let client_ep = endpoint::bind(0).expect("client bind");

        let (initiator_priv, initiator_pub) = generate_static_keypair();
        let (responder_priv, _responder_pub) = generate_static_keypair();
        let (_wrong_priv, wrong_pub) = generate_static_keypair();

        let responder_priv_for_task = responder_priv.clone();
        let initiator_pub_for_task = initiator_pub.clone();
        let server_task = tokio::spawn(async move {
            let incoming = server_ep.accept().await.expect("no incoming connection");
            let connection = incoming.await.expect("server handshake failed");
            let result = ik_derive_shared_secret_responder(
                &connection,
                &responder_priv_for_task,
                &initiator_pub_for_task,
            )
            .await;
            assert!(result.is_err());
        });

        let connection = client_ep
            .connect(server_addr, "zwormhole.local")
            .expect("client connect setup failed")
            .await
            .expect("client handshake failed");


        let _ = ik_derive_shared_secret_initiator(&connection, &initiator_priv, &wrong_pub).await;

        server_task.await.expect("server task panicked");
    }
}
