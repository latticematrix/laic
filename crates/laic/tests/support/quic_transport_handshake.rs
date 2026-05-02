use std::time::Duration;

use super::*;
use prost::Message as ProstMessage;

// WHY: this sibling module keeps handshake integration coverage inside the
// existing `quic_transport` test binary. Splitting into a new integration-test
// binary would reintroduce the Windows Firewall prompt/race that the repo
// explicitly avoids for QUIC coverage.

#[derive(Clone, PartialEq, prost::Message)]
struct TestHello {
    #[prost(uint32, tag = "1")]
    protocol_version: u32,
    #[prost(string, tag = "2")]
    trust_domain: String,
    #[prost(bytes = "vec", tag = "3")]
    client_nonce: Vec<u8>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct TestHelloAck {
    #[prost(uint32, tag = "1")]
    protocol_version: u32,
    #[prost(string, tag = "2")]
    trust_domain: String,
    #[prost(bytes = "vec", tag = "3")]
    client_nonce: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    server_nonce: Vec<u8>,
    #[prost(bytes = "vec", tag = "5")]
    session_id: Vec<u8>,
    #[prost(uint32, tag = "6")]
    rejection_code: u32,
    #[prost(string, optional, tag = "7")]
    rejected_expected_remote_trust_domain: Option<String>,
}

fn sample_test_hello_ack() -> TestHelloAck {
    TestHelloAck {
        protocol_version: u32::from(laic::protocol::constants::VERSION),
        trust_domain: "prod-b".to_string(),
        client_nonce: vec![0u8; 16],
        server_nonce: [9u8; 16].to_vec(),
        session_id: [3u8; 16].to_vec(),
        rejection_code: 0,
        rejected_expected_remote_trust_domain: None,
    }
}

fn hello_ack_message(ack: &TestHelloAck) -> Message {
    Message::new(
        MsgType::CONTROL,
        90_001,
        PayloadFormat::Protobuf,
        Qos::High,
        ack.encode_to_vec(),
    )
}

#[tokio::test]
async fn trust_domain_handshake_roundtrip() {
    let pki = TestPki::generate();
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    let server_task = tokio::spawn(async move {
        let mut conn = server.accept().await.expect("accept");
        let session = server_handshake(
            &mut conn,
            ServerHandshakeConfig::new("prod-a", Some("prod-a")),
        )
        .await
        .expect("server handshake");
        expect_release_heartbeat(&mut conn).await;
        session
    });

    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls)
        .await
        .expect("connect");
    let client_session = client_handshake(
        &mut client,
        ClientHandshakeConfig::new("prod-a", Some("prod-a")),
    )
    .await
    .expect("client handshake");
    send_release_heartbeat(&mut client).await;

    let server_session = server_task.await.expect("join");
    assert_eq!(
        client_session.protocol_version(),
        server_session.protocol_version()
    );
    assert_eq!(client_session.local_trust_domain(), "prod-a");
    assert_eq!(client_session.remote_trust_domain(), "prod-a");
    assert_eq!(server_session.local_trust_domain(), "prod-a");
    assert_eq!(server_session.remote_trust_domain(), "prod-a");
    assert_eq!(client_session.session_id(), server_session.session_id());
}

#[tokio::test]
async fn trust_domain_handshake_rejects_protocol_version_mismatch() {
    let pki = TestPki::generate();
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    let server_task = tokio::spawn(async move {
        let mut conn = server.accept().await.expect("accept");
        let result = server_handshake(
            &mut conn,
            ServerHandshakeConfig::new("prod-a", Some("prod-a")).with_protocol_version(0x0002),
        )
        .await;
        expect_release_heartbeat(&mut conn).await;
        result
            .expect_err("server should reject version mismatch")
            .code()
            .as_u16()
    });

    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls)
        .await
        .expect("connect");
    let err = client_handshake(
        &mut client,
        ClientHandshakeConfig::new("prod-a", Some("prod-a")),
    )
    .await
    .expect_err("client should reject mismatched protocol version");
    send_release_heartbeat(&mut client).await;

    assert_eq!(err.code().as_u16(), 0x0308);
    let server_code = server_task.await.expect("join");
    assert_eq!(server_code, 0x0308);
}

#[tokio::test]
async fn trust_domain_handshake_rejects_remote_domain_mismatch() {
    let pki = TestPki::generate();
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    let server_task = tokio::spawn(async move {
        let mut conn = server.accept().await.expect("accept");
        let session = server_handshake(
            &mut conn,
            ServerHandshakeConfig::new("prod-b", Some("prod-a")),
        )
        .await
        .expect("server handshake");
        expect_release_heartbeat(&mut conn).await;
        session
    });

    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls)
        .await
        .expect("connect");
    let err = client_handshake(
        &mut client,
        ClientHandshakeConfig::new("prod-a", Some("prod-a")),
    )
    .await
    .expect_err("client should reject mismatched remote trust domain");
    send_release_heartbeat(&mut client).await;

    assert_eq!(err.code().as_u16(), 0x0309);
    let _ = server_task.await.expect("join");
}

#[tokio::test]
async fn trust_domain_handshake_rejects_client_domain_mismatch() {
    let pki = TestPki::generate();
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    let server_task = tokio::spawn(async move {
        let mut conn = server.accept().await.expect("accept");
        let result = server_handshake(
            &mut conn,
            ServerHandshakeConfig::new("prod-a", Some("prod-b")),
        )
        .await;
        expect_release_heartbeat(&mut conn).await;
        result
            .expect_err("server should reject mismatched client trust domain")
            .code()
            .as_u16()
    });

    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls)
        .await
        .expect("connect");
    let err = client_handshake(
        &mut client,
        ClientHandshakeConfig::new("prod-a", Some("prod-a")),
    )
    .await
    .expect_err("client should reject when server refuses its claimed trust domain");
    send_release_heartbeat(&mut client).await;

    assert_eq!(err.code().as_u16(), 0x0309);
    let server_code = server_task.await.expect("join");
    assert_eq!(server_code, 0x0309);
}

#[tokio::test]
async fn trust_domain_handshake_rejects_empty_local_domain() {
    let pki = TestPki::generate();
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    let server_task = tokio::spawn(async move {
        // TRAP: empty local-domain validation can fail before the client sends
        // any handshake frame. `QuicServer::accept()` also waits for the first
        // bidirectional stream, so this fixture must tolerate timeout/accept
        // failure instead of treating it as a product regression.
        if let Ok(Ok(mut conn)) =
            tokio::time::timeout(Duration::from_millis(500), server.accept()).await
        {
            let _ = tokio::time::timeout(Duration::from_millis(200), conn.receive()).await;
        }
    });

    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls)
        .await
        .expect("connect");
    let err = client_handshake(&mut client, ClientHandshakeConfig::new("", None))
        .await
        .expect_err("empty local trust domain must fail");

    assert_eq!(err.code().as_u16(), 0x030B);
    server_task.await.expect("join");
}

#[tokio::test]
async fn trust_domain_handshake_rejects_rejection_ack_with_session_material() {
    let pki = TestPki::generate();
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    let server_task = tokio::spawn(async move {
        let mut conn = server.accept().await.expect("accept");
        let hello = conn.receive().await.expect("receive hello");
        let hello = TestHello::decode(hello.payload()).expect("decode hello");

        let mut ack = sample_test_hello_ack();
        ack.client_nonce = hello.client_nonce;
        ack.rejection_code = 0x0309;
        ack.rejected_expected_remote_trust_domain = Some("prod-a".to_string());
        conn.send(&hello_ack_message(&ack)).await.expect("send ack");
        expect_release_heartbeat(&mut conn).await;
    });

    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls)
        .await
        .expect("connect");
    let err = client_handshake(
        &mut client,
        ClientHandshakeConfig::new("prod-a", Some("prod-b")),
    )
    .await
    .expect_err("rejection ack must not carry session material");
    send_release_heartbeat(&mut client).await;

    assert_eq!(err.code().as_u16(), 0x030B);
    server_task.await.expect("join");
}

#[tokio::test]
async fn trust_domain_handshake_rejects_malformed_ack_payload_as_protocol_error() {
    let pki = TestPki::generate();
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    let server_task = tokio::spawn(async move {
        let mut conn = server.accept().await.expect("accept");
        let _ = conn.receive().await.expect("receive hello");
        let malformed_ack = Message::new(
            MsgType::CONTROL,
            90_099,
            PayloadFormat::Protobuf,
            Qos::High,
            vec![0x08],
        );
        conn.send(&malformed_ack).await.expect("send malformed ack");
        expect_release_heartbeat(&mut conn).await;
    });

    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls)
        .await
        .expect("connect");
    let err = client_handshake(
        &mut client,
        ClientHandshakeConfig::new("prod-a", Some("prod-b")),
    )
    .await
    .expect_err("malformed ack payload must stay in protocol layer");
    send_release_heartbeat(&mut client).await;

    assert_eq!(err.code().as_u16(), 0x030B);
    server_task.await.expect("join");
}

#[tokio::test]
async fn trust_domain_handshake_rejects_unknown_rejection_code() {
    let pki = TestPki::generate();
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    let server_task = tokio::spawn(async move {
        let mut conn = server.accept().await.expect("accept");
        let hello = conn.receive().await.expect("receive hello");
        let hello = TestHello::decode(hello.payload()).expect("decode hello");

        let mut ack = sample_test_hello_ack();
        ack.client_nonce = hello.client_nonce;
        ack.server_nonce.clear();
        ack.session_id.clear();
        ack.rejection_code = 0x03FE;
        conn.send(&hello_ack_message(&ack)).await.expect("send ack");
        expect_release_heartbeat(&mut conn).await;
    });

    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls)
        .await
        .expect("connect");
    let err = client_handshake(
        &mut client,
        ClientHandshakeConfig::new("prod-a", Some("prod-b")),
    )
    .await
    .expect_err("unknown rejection code must stay in protocol layer");
    send_release_heartbeat(&mut client).await;

    assert_eq!(err.code().as_u16(), 0x030B);
    server_task.await.expect("join");
}

#[tokio::test]
async fn trust_domain_handshake_rejects_success_ack_with_rejection_explanation() {
    let pki = TestPki::generate();
    let server = bind_server(&pki.server_tls);
    let addr = server.local_addr().expect("local addr");

    let server_task = tokio::spawn(async move {
        let mut conn = server.accept().await.expect("accept");
        let hello = conn.receive().await.expect("receive hello");
        let hello = TestHello::decode(hello.payload()).expect("decode hello");

        let mut ack = sample_test_hello_ack();
        ack.client_nonce = hello.client_nonce;
        ack.rejected_expected_remote_trust_domain = Some("prod-c".to_string());
        conn.send(&hello_ack_message(&ack)).await.expect("send ack");
        expect_release_heartbeat(&mut conn).await;
    });

    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls)
        .await
        .expect("connect");
    let result = client_handshake(
        &mut client,
        ClientHandshakeConfig::new("prod-a", Some("prod-b")),
    )
    .await;
    send_release_heartbeat(&mut client).await;

    let err = result.expect_err("success ack carrying rejection explanation must fail");
    assert_eq!(err.code().as_u16(), 0x030B);
    server_task.await.expect("join");
}

#[tokio::test]
async fn trust_domain_handshake_distinguishes_protocol_rejection_from_transport_teardown() {
    let pki = TestPki::generate();

    let protocol_server = bind_server(&pki.server_tls);
    let protocol_addr = protocol_server.local_addr().expect("protocol addr");
    let protocol_task = tokio::spawn(async move {
        let mut conn = protocol_server.accept().await.expect("accept");
        let hello = conn.receive().await.expect("receive hello");
        let hello = TestHello::decode(hello.payload()).expect("decode hello");

        let mut ack = sample_test_hello_ack();
        ack.client_nonce = hello.client_nonce;
        ack.server_nonce.clear();
        ack.session_id.clear();
        ack.rejection_code = 0x03FE;
        conn.send(&hello_ack_message(&ack)).await.expect("send ack");
        expect_release_heartbeat(&mut conn).await;
    });

    let mut protocol_client = QuicConnection::connect(protocol_addr, "localhost", &pki.client_tls)
        .await
        .expect("connect");
    let protocol_err = client_handshake(
        &mut protocol_client,
        ClientHandshakeConfig::new("prod-a", Some("prod-b")),
    )
    .await
    .expect_err("malformed rejection ack must stay in protocol layer");
    send_release_heartbeat(&mut protocol_client).await;

    assert_eq!(protocol_err.code().as_u16(), 0x030B);
    protocol_task.await.expect("join");

    let teardown_server = bind_server(&pki.server_tls);
    let teardown_addr = teardown_server.local_addr().expect("teardown addr");
    let teardown_task = tokio::spawn(async move {
        let mut conn = teardown_server.accept().await.expect("accept");
        let _ = conn.receive().await.expect("receive hello");
        // WHY: exiting right after reading `Hello` simulates the exact fixture
        // bug that `expect_release_heartbeat()` prevents. No ack is sent, so
        // the client should observe a transport failure rather than a stable
        // protocol rejection.
    });

    let mut teardown_client = QuicConnection::connect(teardown_addr, "localhost", &pki.client_tls)
        .await
        .expect("connect");
    let teardown_err = client_handshake(
        &mut teardown_client,
        ClientHandshakeConfig::new("prod-a", Some("prod-b")),
    )
    .await
    .expect_err("task teardown must surface as a transport-layer failure");

    assert!(
        matches!(teardown_err, laic::LaicError::Transport(_)),
        "expected transport-layer error, got {teardown_err:?}"
    );
    teardown_task.await.expect("join");
}
