//! Shared helpers for `quic_transport.rs`.
//!
//! WHY: QUIC, gateway, and trust-domain handshake tests intentionally stay in
//! the same integration-test binary because Windows Firewall may treat each
//! test binary as a separate program. Keeping the helpers in this sibling
//! module preserves that single-binary constraint while letting the entry file
//! stay below the repo's 500-line gate.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};

use laic::transport::ipc::IpcConnection;
use laic::transport::quic::{QuicConnection, QuicServer};
use laic::transport::tls::{ClientTlsConfig, ServerTlsConfig};
use laic::{Gateway, Message, MsgType, PayloadFormat, Qos};
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// Test PKI: generates a CA + server cert + client cert for mTLS tests.
pub(crate) struct TestPki {
    pub(crate) ca_cert_der: CertificateDer<'static>,
    pub(crate) server_tls: ServerTlsConfig,
    pub(crate) client_tls: ClientTlsConfig,
}

impl TestPki {
    pub(crate) fn generate() -> Self {
        // CA (self-signed)
        let mut ca_params = CertificateParams::new(Vec::<String>::new()).expect("CA params");
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "LAIC Test CA");
        let ca_key = KeyPair::generate().expect("CA key");
        let ca_cert = ca_params.self_signed(&ca_key).expect("CA cert");
        let ca_cert_der = CertificateDer::from(ca_cert.der().to_vec());

        // Server cert signed by CA
        let server_params =
            CertificateParams::new(vec!["localhost".to_string()]).expect("server params");
        let server_key = KeyPair::generate().expect("server key");
        let server_cert = server_params
            .signed_by(&server_key, &ca_cert, &ca_key)
            .expect("server cert");
        let server_cert_der = CertificateDer::from(server_cert.der().to_vec());
        let server_key_der =
            PrivateKeyDer::from(PrivatePkcs8KeyDer::from(server_key.serialize_der()));

        // Client cert signed by CA
        let client_params =
            CertificateParams::new(vec!["laic-test-client".to_string()]).expect("client params");
        let client_key = KeyPair::generate().expect("client key");
        let client_cert = client_params
            .signed_by(&client_key, &ca_cert, &ca_key)
            .expect("client cert");
        let client_cert_der = CertificateDer::from(client_cert.der().to_vec());
        let client_key_der =
            PrivateKeyDer::from(PrivatePkcs8KeyDer::from(client_key.serialize_der()));

        let server_tls =
            ServerTlsConfig::new(vec![server_cert_der], server_key_der, ca_cert_der.clone());

        let client_tls =
            ClientTlsConfig::new(ca_cert_der.clone(), vec![client_cert_der], client_key_der);

        Self {
            ca_cert_der,
            server_tls,
            client_tls,
        }
    }
}

pub(crate) fn sample_message(payload: Vec<u8>) -> Message {
    Message::new(
        MsgType::DATA,
        42,
        PayloadFormat::Arrow,
        Qos::Normal,
        payload,
    )
}

/// Bind a server on localhost with a random port.
pub(crate) fn bind_server(tls: &ServerTlsConfig) -> QuicServer {
    let addr: SocketAddr = "127.0.0.1:0".parse().expect("addr");
    QuicServer::bind(addr, tls).expect("bind")
}

pub(crate) async fn expect_release_heartbeat(conn: &mut QuicConnection) {
    // TRAP: the server-side handshake task must stay alive until the client has
    // consumed `HelloAck`. If the task returns immediately, dropping the QUIC
    // connection races with the client read and the test fails as a misleading
    // transport-level `connection lost` instead of the real protocol outcome.
    // The paired teardown-race regression test intentionally omits this helper
    // in its second sub-case to prove that this fixture choreography matters.
    let release = conn.receive().await.expect("release receive");
    assert_eq!(release.msg_type(), MsgType::HEARTBEAT);
}

pub(crate) async fn send_release_heartbeat(conn: &mut QuicConnection) {
    // WHY: mirror `expect_release_heartbeat()` so the peer can keep the QUIC
    // connection open long enough to consume `HelloAck` before the server task
    // exits and drops the transport.
    let release = Message::new(
        MsgType::HEARTBEAT,
        0,
        PayloadFormat::Raw,
        Qos::Normal,
        vec![],
    );
    conn.send(&release).await.expect("release send");
}

static GW_COUNTER: AtomicU32 = AtomicU32::new(7000);

fn gw_unique_name() -> String {
    let id = GW_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("gw/{id}")
}

/// Set up a Gateway with IPC and QUIC peers.
///
/// Returns `(gateway, local_ipc_peer, remote_quic_peer)`.
pub(crate) async fn setup_gateway() -> (Gateway, IpcConnection, QuicConnection) {
    let pki = TestPki::generate();

    // QUIC pair.
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    let accept_handle = tokio::spawn(async move { server.accept().await.expect("accept") });

    let mut gateway_quic = QuicConnection::connect(addr, "localhost", &pki.client_tls)
        .await
        .expect("connect");

    // PITFALL: Quinn on Windows may not flush the STREAM frame from
    // open_bi() until data is actually written to the stream. Send a
    // heartbeat to force stream establishment so accept_bi() resolves.
    let heartbeat = Message::new(
        MsgType::HEARTBEAT,
        0,
        PayloadFormat::Raw,
        Qos::Normal,
        vec![],
    );
    gateway_quic.send(&heartbeat).await.expect("heartbeat");

    let mut remote_peer = accept_handle.await.expect("join");

    // Consume the heartbeat on the server side.
    let hb = remote_peer.receive().await.expect("consume heartbeat");
    assert_eq!(hb.msg_type(), MsgType::HEARTBEAT);

    // IPC pair.
    let name = gw_unique_name();
    let gateway_ipc = IpcConnection::open_server(&name).expect("open ipc server");
    let local_peer = IpcConnection::open_client(&name).expect("open ipc client");

    let gateway = Gateway::new(gateway_ipc, gateway_quic);
    (gateway, local_peer, remote_peer)
}
