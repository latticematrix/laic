//! IPC transport integration tests.
//!
//! Validates shared-memory message roundtrip, bidirectional exchange,
//! error handling, and the close lifecycle using iceoryx2.

use std::sync::atomic::{AtomicU32, Ordering};

use laic::transport::ipc::{IpcConnection, MAX_IPC_PAYLOAD_LEN};
use laic::{Message, MsgType, PayloadFormat, Qos};

// ---------------------------------------------------------------------------
// Unique service name generation
// ---------------------------------------------------------------------------

/// Global counter to generate unique service names per test, avoiding
/// iceoryx2 service name collisions when tests run in parallel.
static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn unique_name() -> String {
    let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("test/{id}")
}

fn sample_message(msg_id: u64, payload: Vec<u8>) -> Message {
    Message::new(
        MsgType::DATA,
        msg_id,
        PayloadFormat::Arrow,
        Qos::Normal,
        payload,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn single_message_roundtrip() {
    let name = unique_name();
    let mut server = IpcConnection::open_server(&name).expect("open server");
    let mut client = IpcConnection::open_client(&name).expect("open client");

    let msg = sample_message(1, vec![0xAA, 0xBB, 0xCC, 0xDD]);
    client.send(&msg).await.expect("client send");

    let received = server.receive().await.expect("server receive");
    assert_eq!(received.header().msg_id, msg.header().msg_id);
    assert_eq!(received.header().msg_type, msg.header().msg_type);
    assert_eq!(received.payload(), msg.payload());
}

#[tokio::test]
async fn multiple_messages_sequential() {
    let name = unique_name();
    let mut server = IpcConnection::open_server(&name).expect("open server");
    let mut client = IpcConnection::open_client(&name).expect("open client");

    for i in 0u8..3 {
        let payload = vec![i; (i as usize + 1) * 10];
        let msg = sample_message(u64::from(i), payload);
        client.send(&msg).await.expect("client send");
    }

    for i in 0u8..3 {
        let received = server.receive().await.expect("server receive");
        let expected_len = (i as usize + 1) * 10;
        assert_eq!(received.header().msg_id, u64::from(i));
        assert_eq!(received.payload().len(), expected_len);
        assert!(received.payload().iter().all(|&b| b == i));
    }
}

#[tokio::test]
async fn bidirectional_exchange() {
    let name = unique_name();
    let mut server = IpcConnection::open_server(&name).expect("open server");
    let mut client = IpcConnection::open_client(&name).expect("open client");

    // Client → Server
    let request = sample_message(10, vec![1, 2, 3]);
    client.send(&request).await.expect("client send");

    let received_request = server.receive().await.expect("server receive");
    assert_eq!(received_request.payload(), &[1, 2, 3]);

    // Server → Client
    let reply = Message::new(
        MsgType::ACK,
        10,
        PayloadFormat::Raw,
        Qos::Normal,
        vec![0xFF],
    );
    server.send(&reply).await.expect("server send");

    let received_reply = client.receive().await.expect("client receive");
    assert_eq!(received_reply.header().msg_type, MsgType::ACK.as_u16());
    assert_eq!(received_reply.payload(), &[0xFF]);
}

#[tokio::test]
async fn empty_payload_roundtrip() {
    let name = unique_name();
    let mut server = IpcConnection::open_server(&name).expect("open server");
    let mut client = IpcConnection::open_client(&name).expect("open client");

    let msg = Message::new(
        MsgType::HEARTBEAT,
        99,
        PayloadFormat::Raw,
        Qos::Normal,
        vec![],
    );
    client.send(&msg).await.expect("client send");

    let received = server.receive().await.expect("server receive");
    assert_eq!(received.header().msg_id, 99);
    assert_eq!(received.header().payload_len, 0);
    assert!(received.payload().is_empty());
}

#[tokio::test]
async fn payload_exceeds_ipc_max() {
    let name = unique_name();
    let mut client = IpcConnection::open_client(&name).expect("open client");

    let oversized = vec![0u8; MAX_IPC_PAYLOAD_LEN as usize + 1];
    let msg = sample_message(1, oversized);
    let err = client
        .send(&msg)
        .await
        .expect_err("should reject oversized");

    // FramingError = 0x0109
    assert_eq!(err.code().as_u16(), 0x0109);
}

#[tokio::test]
async fn send_after_close() {
    let name = unique_name();
    let mut client = IpcConnection::open_client(&name).expect("open client");

    client.close().await.expect("close");

    let msg = sample_message(1, vec![1, 2, 3]);
    let err = client
        .send(&msg)
        .await
        .expect_err("should reject after close");

    // ShuttingDown = 0x0108
    assert_eq!(err.code().as_u16(), 0x0108);
}

#[tokio::test]
async fn receive_after_close() {
    let name = unique_name();
    let mut server = IpcConnection::open_server(&name).expect("open server");

    server.close().await.expect("close");

    let err = server
        .receive()
        .await
        .expect_err("should reject after close");

    // ShuttingDown = 0x0108
    assert_eq!(err.code().as_u16(), 0x0108);
}
