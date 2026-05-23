//! QUIC transport integration tests.
//!
//! Validates mTLS handshake, message roundtrip, and certificate
//! rejection using a test PKI (self-signed CA + signed certs).
//!
//! Also hosts Gateway integration tests that require both IPC and QUIC,
//! because each test binary is a separate Windows executable and Windows
//! Firewall may block new binaries from opening UDP sockets. Keeping
//! gateway tests here reuses the already-allowed test binary.

use std::time::Duration;

#[path = "support/quic_transport_handshake.rs"]
mod quic_transport_handshake;
#[path = "support/quic_transport.rs"]
mod quic_transport_support;

use laic::handshake::{
    client_handshake, server_handshake, ClientHandshakeConfig, ServerHandshakeConfig,
};
use laic::transport::quic::QuicConnection;
use laic::transport::tls::ClientTlsConfig;
use laic::{Message, MsgType, PayloadFormat, Qos};
use quic_transport_support::{
    bind_server, expect_release_heartbeat, sample_message, send_release_heartbeat, setup_gateway,
    TestPki,
};
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

async fn connected_pair() -> (QuicConnection, QuicConnection) {
    let pki = TestPki::generate();
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    let server_handle = tokio::spawn(async move { server.accept().await.expect("server accept") });

    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls)
        .await
        .expect("client connect");

    let seed = sample_message(vec![0xCC]);
    client.send(&seed).await.expect("seed send");

    let mut server_conn = server_handle.await.expect("join");
    let received = server_conn.receive().await.expect("seed receive");
    assert_eq!(received.payload(), seed.payload());

    (client, server_conn)
}

#[tokio::test]
async fn mtls_handshake_and_roundtrip() {
    let pki = TestPki::generate();
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    // Server accepts in background.
    let server_handle = tokio::spawn(async move { server.accept().await.expect("server accept") });

    // Client connects and sends a message.
    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls)
        .await
        .expect("client connect");
    let msg = sample_message(vec![1, 2, 3, 4, 5]);
    client.send(&msg).await.expect("client send");

    // Server receives.
    let mut server_conn = server_handle.await.expect("join");
    let received = server_conn.receive().await.expect("server receive");
    assert_eq!(received.header(), msg.header());
    assert_eq!(received.payload(), msg.payload());
}

#[tokio::test]
async fn bidirectional_exchange() {
    let pki = TestPki::generate();
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    let server_handle = tokio::spawn(async move {
        let mut conn = server.accept().await.expect("accept");
        // Server receives, then responds.
        let msg = conn.receive().await.expect("server recv");
        let reply = Message::new(
            MsgType::ACK,
            msg.header().msg_id,
            PayloadFormat::Raw,
            Qos::Normal,
            vec![0xAA],
        );
        conn.send(&reply).await.expect("server send");
        conn
    });

    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls)
        .await
        .expect("connect");

    // Client sends request.
    let request = sample_message(vec![10, 20, 30]);
    client.send(&request).await.expect("client send");

    // Client receives reply.
    let reply = client.receive().await.expect("client recv");
    assert_eq!(reply.header().msg_type, MsgType::ACK.as_u16());
    assert_eq!(reply.payload(), &[0xAA]);

    let _ = server_handle.await;
}

#[tokio::test]
async fn multiple_messages_sequential() {
    let pki = TestPki::generate();
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    let server_handle = tokio::spawn(async move {
        let mut conn = server.accept().await.expect("accept");
        let mut messages = Vec::new();
        for _ in 0..5 {
            messages.push(conn.receive().await.expect("recv"));
        }
        messages
    });

    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls)
        .await
        .expect("connect");

    for i in 0u8..5 {
        let msg = sample_message(vec![i; (i as usize + 1) * 10]);
        client.send(&msg).await.expect("send");
    }

    let messages = server_handle.await.expect("join");
    assert_eq!(messages.len(), 5);
    for (i, msg) in messages.iter().enumerate() {
        let expected_len = (i + 1) * 10;
        assert_eq!(msg.payload().len(), expected_len);
        assert!(msg.payload().iter().all(|&b| b == i as u8));
    }
}

#[tokio::test]
async fn untrusted_client_cert_rejected() {
    let pki = TestPki::generate();
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    // Generate a rogue client cert signed by a DIFFERENT CA.
    let mut rogue_ca_params = CertificateParams::new(Vec::<String>::new()).expect("params");
    rogue_ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    rogue_ca_params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "Rogue CA");
    let rogue_ca_key = KeyPair::generate().expect("key");
    let rogue_ca_cert = rogue_ca_params.self_signed(&rogue_ca_key).expect("cert");

    let rogue_params = CertificateParams::new(vec!["rogue-client".to_string()]).expect("params");
    let rogue_key = KeyPair::generate().expect("key");
    let rogue_cert = rogue_params
        .signed_by(&rogue_key, &rogue_ca_cert, &rogue_ca_key)
        .expect("cert");
    let rogue_cert_der = CertificateDer::from(rogue_cert.der().to_vec());
    let rogue_key_der = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(rogue_key.serialize_der()));

    // Client with rogue cert trusts the real server CA.
    let rogue_client_tls =
        ClientTlsConfig::new(pki.ca_cert_der.clone(), vec![rogue_cert_der], rogue_key_der);

    let server_handle = tokio::spawn(async move {
        // Server accept should fail — rogue client cert not trusted.
        server.accept().await
    });

    // WHY: in TLS 1.3, client cert verification is asynchronous.
    // The client may successfully connect() and open_bi(), but the
    // server will reject the handshake. The failure surfaces when
    // the client tries to communicate (send/receive) or when the
    // server's accept result is checked.
    let client_result = QuicConnection::connect(addr, "localhost", &rogue_client_tls).await;

    // The failure may surface at connect, send, or on the server side.
    let failed = if let Ok(mut conn) = client_result {
        // Connection appeared to succeed from client side — try to
        // communicate. The server rejection will cause a failure.
        let msg = sample_message(vec![1, 2, 3]);
        let send_result = conn.send(&msg).await;
        if send_result.is_err() {
            true
        } else {
            // Send buffered locally; check server side.
            let server_result = server_handle.await.expect("join");
            server_result.is_err()
        }
    } else {
        true
    };

    assert!(failed, "mTLS with untrusted client cert must fail");
}

#[tokio::test]
async fn connection_close() {
    let pki = TestPki::generate();
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    let server_handle = tokio::spawn(async move {
        let conn = server.accept().await.expect("accept");
        conn
    });

    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls)
        .await
        .expect("connect");

    // Send a message so server can accept_bi.
    let msg = sample_message(vec![42]);
    client.send(&msg).await.expect("send");

    let _ = server_handle.await;

    // Client closes gracefully.
    client.close().await.expect("close");
}

#[tokio::test]
async fn send_after_close_returns_shutting_down() {
    let (mut client, _server_conn) = connected_pair().await;

    client.close().await.expect("close");

    let msg = sample_message(vec![0x01]);
    let err = client
        .send(&msg)
        .await
        .expect_err("send after close should be rejected");

    // ShuttingDown = 0x0108
    assert_eq!(err.code().as_u16(), 0x0108);
}

#[tokio::test]
async fn receive_after_close_returns_shutting_down() {
    let (mut client, _server_conn) = connected_pair().await;

    client.close().await.expect("close");

    let err = tokio::time::timeout(Duration::from_secs(2), client.receive())
        .await
        .expect("receive after close should not hang")
        .expect_err("receive after close should be rejected");

    // ShuttingDown = 0x0108
    assert_eq!(err.code().as_u16(), 0x0108);
}

// ---------------------------------------------------------------------------
// Gateway integration tests (IPC ↔ QUIC bridge)
// ---------------------------------------------------------------------------

/// IPC → Gateway → QUIC: local client sends via IPC, remote server
/// receives via QUIC.
#[tokio::test]
async fn gateway_forward_ipc_to_quic() {
    let (mut gateway, mut local_peer, mut remote_peer) = setup_gateway().await;

    let msg = Message::new(
        MsgType::DATA,
        101,
        PayloadFormat::Arrow,
        Qos::Normal,
        vec![0xAA, 0xBB, 0xCC],
    );
    local_peer.send(&msg).await.expect("local send");

    gateway
        .forward_ipc_to_quic()
        .await
        .expect("forward ipc→quic");

    let received = remote_peer.receive().await.expect("remote receive");
    assert_eq!(received.header().msg_id, 101);
    assert_eq!(received.payload(), &[0xAA, 0xBB, 0xCC]);
}

/// QUIC → Gateway → IPC: remote server sends via QUIC, local client
/// receives via IPC.
#[tokio::test]
async fn gateway_forward_quic_to_ipc() {
    let (mut gateway, mut local_peer, mut remote_peer) = setup_gateway().await;

    let msg = Message::new(
        MsgType::ACK,
        202,
        PayloadFormat::Raw,
        Qos::High,
        vec![0x11, 0x22],
    );
    remote_peer.send(&msg).await.expect("remote send");

    gateway
        .forward_quic_to_ipc()
        .await
        .expect("forward quic→ipc");

    let received = local_peer.receive().await.expect("local receive");
    assert_eq!(received.header().msg_id, 202);
    assert_eq!(received.payload(), &[0x11, 0x22]);
}

/// After close(), forwarding must fail with ShuttingDown.
#[tokio::test]
async fn gateway_close_then_forward_rejected() {
    let (mut gateway, _local_peer, _remote_peer) = setup_gateway().await;

    gateway.close().await.expect("close");

    // forward_ipc_to_quic tries self.ipc.receive() first, which returns
    // ShuttingDown because IPC is closed.
    let err = gateway
        .forward_ipc_to_quic()
        .await
        .expect_err("should reject after close");

    // ShuttingDown = 0x0108
    assert_eq!(err.code().as_u16(), 0x0108);
}

/// into_parts decomposes the gateway and both connections remain usable.
#[tokio::test]
async fn gateway_into_parts() {
    let (gateway, mut local_peer, mut remote_peer) = setup_gateway().await;

    let (mut ipc, mut quic) = gateway.into_parts();

    // IPC side still works: local_peer → ipc.
    let msg = Message::new(
        MsgType::DATA,
        301,
        PayloadFormat::Raw,
        Qos::Normal,
        vec![0xDD],
    );
    local_peer.send(&msg).await.expect("local send");
    let received = ipc.receive().await.expect("ipc receive");
    assert_eq!(received.header().msg_id, 301);

    // QUIC side still works: quic → remote_peer.
    quic.send(&msg).await.expect("quic send");
    let received = remote_peer.receive().await.expect("remote receive");
    assert_eq!(received.payload(), &[0xDD]);
}
