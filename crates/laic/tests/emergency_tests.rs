//! Integration tests for the LAIC emergency channel.

use std::sync::atomic::{AtomicU32, Ordering};

use laic::emergency::MAX_EMERGENCY_PAYLOAD_LEN;
use laic::EmergencyChannel;

/// Unique test name generator to avoid iceoryx2 service name collisions.
fn test_name(label: &str) -> String {
    static CTR: AtomicU32 = AtomicU32::new(0);
    let id = CTR.fetch_add(1, Ordering::Relaxed);
    format!("emergency-test/{label}/{id}")
}

#[tokio::test]
async fn roundtrip() {
    let name = test_name("roundtrip");
    let mut server = EmergencyChannel::open_server(&name).unwrap();
    let mut client = EmergencyChannel::open_client(&name).unwrap();

    let payload = b"LP7-EMERGENCY-ALERT";
    client.send(payload).await.unwrap();

    let received = server.receive().await.unwrap();
    assert_eq!(received, payload);
}

#[tokio::test]
async fn max_payload() {
    let name = test_name("max-payload");
    let mut server = EmergencyChannel::open_server(&name).unwrap();
    let mut client = EmergencyChannel::open_client(&name).unwrap();

    let payload = vec![0xAB; MAX_EMERGENCY_PAYLOAD_LEN];
    client.send(&payload).await.unwrap();

    let received = server.receive().await.unwrap();
    assert_eq!(received.len(), MAX_EMERGENCY_PAYLOAD_LEN);
    assert_eq!(received, payload);
}

#[tokio::test]
async fn empty_payload() {
    let name = test_name("empty");
    let mut server = EmergencyChannel::open_server(&name).unwrap();
    let mut client = EmergencyChannel::open_client(&name).unwrap();

    client.send(b"").await.unwrap();

    let received = server.receive().await.unwrap();
    assert!(received.is_empty());
}

#[tokio::test]
async fn oversized_payload_rejected() {
    let name = test_name("oversized");
    let mut client = EmergencyChannel::open_client(&name).unwrap();

    let oversized = vec![0xFF; MAX_EMERGENCY_PAYLOAD_LEN + 1];
    let err = client.send(&oversized).await.unwrap_err();
    // FramingError = 0x0109
    assert_eq!(err.code().as_u16(), 0x0109);
}

#[tokio::test]
async fn multiple_sequential() {
    let name = test_name("multi-seq");
    let mut server = EmergencyChannel::open_server(&name).unwrap();
    let mut client = EmergencyChannel::open_client(&name).unwrap();

    for i in 0u8..5 {
        client.send(&[i; 10]).await.unwrap();
    }

    for i in 0u8..5 {
        let received = server.receive().await.unwrap();
        assert_eq!(received, vec![i; 10]);
    }
}

#[tokio::test]
async fn bidirectional() {
    let name = test_name("bidir");
    let mut server = EmergencyChannel::open_server(&name).unwrap();
    let mut client = EmergencyChannel::open_client(&name).unwrap();

    // Client → Server
    client.send(b"c2s").await.unwrap();
    let from_client = server.receive().await.unwrap();
    assert_eq!(from_client, b"c2s");

    // Server → Client
    server.send(b"s2c").await.unwrap();
    let from_server = client.receive().await.unwrap();
    assert_eq!(from_server, b"s2c");
}

#[tokio::test]
async fn send_after_close() {
    let name = test_name("send-close");
    let mut client = EmergencyChannel::open_client(&name).unwrap();

    client.close().await.unwrap();
    let err = client.send(b"nope").await.unwrap_err();
    // ShuttingDown = 0x0108
    assert_eq!(err.code().as_u16(), 0x0108);
}

#[tokio::test]
async fn receive_after_close() {
    let name = test_name("recv-close");
    let mut server = EmergencyChannel::open_server(&name).unwrap();

    server.close().await.unwrap();
    let err = server.receive().await.unwrap_err();
    // ShuttingDown = 0x0108
    assert_eq!(err.code().as_u16(), 0x0108);
}
