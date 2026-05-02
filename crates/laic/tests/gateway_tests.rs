//! Gateway integration tests (IPC-only subset).
//!
//! Tests that require both IPC and QUIC live in `quic_transport.rs`
//! because each test binary is a separate Windows executable, and
//! Windows Firewall may block new binaries from opening UDP sockets.
//! `quic_transport.rs` is already firewall-allowed.

use std::sync::atomic::{AtomicU32, Ordering};

use laic::transport::ipc::IpcConnection;
use laic::{Message, MsgType, PayloadFormat, Qos};

// ---------------------------------------------------------------------------
// Test infrastructure
// ---------------------------------------------------------------------------

static GW_IPC_COUNTER: AtomicU32 = AtomicU32::new(6000);

fn unique_name() -> String {
    let id = GW_IPC_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("gw-ipc/{id}")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Verify IPC-side close behavior independent of QUIC.
///
/// After close(), the IPC connection is marked as closed. Any
/// subsequent send/receive on it returns ShuttingDown.
#[tokio::test]
async fn ipc_close_blocks_operations() {
    let name = unique_name();
    let mut server = IpcConnection::open_server(&name).expect("open server");
    let mut client = IpcConnection::open_client(&name).expect("open client");

    // Close the server side (simulating what Gateway.close() does to IPC).
    server.close().await.expect("close");

    // Send from client still works (client isn't closed).
    let msg = Message::new(
        MsgType::DATA,
        1,
        PayloadFormat::Raw,
        Qos::Normal,
        vec![1, 2, 3],
    );
    client.send(&msg).await.expect("client send");

    // But receive on closed server returns ShuttingDown.
    let err = server
        .receive()
        .await
        .expect_err("should reject after close");
    assert_eq!(err.code().as_u16(), 0x0108); // ShuttingDown
}
