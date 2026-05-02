//! IPC transport backend: zero-copy message delivery via shared memory.
//!
//! Uses [iceoryx2](https://docs.rs/iceoryx2) with `ipc_threadsafe::Service`
//! for Send+Sync publisher/subscriber ports. Each [`IpcConnection`] wraps
//! two iceoryx2 pub/sub services forming a bidirectional channel:
//!
//! ```text
//! Server side:
//!   "laic/ipc/{name}/c2s" — subscriber (receives client messages)
//!   "laic/ipc/{name}/s2c" — publisher  (sends to client)
//!
//! Client side:
//!   "laic/ipc/{name}/c2s" — publisher  (sends to server)
//!   "laic/ipc/{name}/s2c" — subscriber (receives server messages)
//! ```
//!
//! # Connection Model
//!
//! CONSTRAINT: each `name` identifies a **1:1 channel** between exactly
//! one server and one client. Multiple clients sharing the same `name`
//! is unsupported and will cause message cross-delivery, because the
//! underlying iceoryx2 pub/sub services are topic-based. Callers must
//! assign a unique `name` per logical connection.
//!
//! # Delivery Semantics
//!
//! CONSTRAINT: IPC delivery is **best-effort**. A successful `send()`
//! means the message has been published to the SHM slot; it does **not**
//! guarantee the subscriber has received it. If the subscriber's buffer
//! is full (16 slots), the oldest unread message is silently replaced
//! (`enable_safe_overflow`). End-to-end reliability requires upper-layer
//! flow control (Phase 4).
//!
//! # D4 Evolution
//!
//! Original ADR D4 assumed `ipc::Service` (!Send) and designed a bridge
//! thread + mpsc channel to cross the async boundary. Phase 3B-2 spike
//! discovered `ipc_threadsafe::Service` provides Send+Sync, eliminating
//! the need for bridge threads. The IPC backend now uses direct async
//! polling: non-blocking `receive()` with an optional bounded active-poll
//! budget before falling back to `tokio::time::sleep(100μs)`.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use iceoryx2::port::update_connections::UpdateConnections;
use iceoryx2::prelude::*;

use crate::error::{LaicError, TransportError};
use crate::protocol::constants::HEADER_SIZE;
use crate::protocol::header::MessageHeader;
use crate::protocol::message::Message;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum payload length for IPC transport (64 KB).
///
/// CONSTRAINT: iceoryx2 SHM slot size is fixed at service creation time.
/// 64 KB covers >99% of typical AI communication payloads (embeddings,
/// control messages). Larger payloads require Phase 4 fragmentation.
pub const MAX_IPC_PAYLOAD_LEN: u32 = 65_536;

/// Polling interval for the receive loop.
///
/// TRADEOFF: this is the transport-wide default backoff. Windows benchmark
/// harnesses may opt into an active-yield window with
/// `LAIC_IPC_ACTIVE_POLL_BUDGET_MS`, but the default receive path must stay
/// conservative until there is cross-platform and idle-path evidence.
const POLL_INTERVAL: Duration = Duration::from_micros(100);

/// Optional active-poll budget environment variable, in milliseconds.
///
/// WHY: the 2026-04-30 Windows-local validation investigation proved a timer
/// rounding trap for hot cross-process IPC benchmarks, not a universal
/// transport policy. Keep active polling opt-in so production/default callers
/// and other platforms do not inherit a CPU/latency tradeoff that they have
/// not explicitly selected.
const ACTIVE_POLL_BUDGET_MS_ENV: &str = "LAIC_IPC_ACTIVE_POLL_BUDGET_MS";

// WHY: the opt-in knob is process configuration. Cache it so the default
// receive hot path does not pay an environment lookup on every call.
static ACTIVE_POLL_BUDGET: OnceLock<Option<Duration>> = OnceLock::new();
/// Subscriber buffer depth (number of `IpcFrame`s the subscriber can queue).
///
/// WHY: iceoryx2 default is 1, meaning rapid sends overflow silently.
/// 16 slots × ~65 KB ≈ 1 MB per direction — acceptable for burst traffic.
const SUBSCRIBER_BUFFER_SIZE: usize = 16;

// ---------------------------------------------------------------------------
// IpcFrame — SHM transmission unit
// ---------------------------------------------------------------------------

/// SHM transport frame: 40-byte encoded header + 64 KB payload buffer.
///
/// WHY: iceoryx2 SHM slots require fixed-size, `Copy` types with
/// `ZeroCopySend`. Uses `[u8; 40]` instead of `MessageHeader` because
/// `MessageHeader` carries `u64` fields that make it non-trivially
/// constructible in const context, and `ZeroCopySend` needs `Copy`.
/// The header's `payload_len` field (inside the encoded bytes) is
/// the single source of truth for valid payload length.
///
/// TRADEOFF: each `IpcFrame` is ~65 KB on the stack during `send()`.
/// Simpler than in-place SHM writes; optimize later if profiling
/// shows this matters.
#[derive(Clone, Copy, ZeroCopySend)]
#[repr(C)]
pub(crate) struct IpcFrame {
    header: [u8; HEADER_SIZE],
    payload: [u8; MAX_IPC_PAYLOAD_LEN as usize],
}

// WHY: manual Debug because the default derive would dump 65536+ bytes
// to output. We show only the header and the payload length hint.
impl core::fmt::Debug for IpcFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IpcFrame")
            .field("header", &self.header)
            .field("payload", &format_args!("[u8; {}]", self.payload.len()))
            .finish()
    }
}

// ---------------------------------------------------------------------------
// IpcConnection
// ---------------------------------------------------------------------------

/// A bidirectional 1:1 IPC connection for exchanging LAIC messages via
/// shared memory.
///
/// Created via [`IpcConnection::open_server`] or
/// [`IpcConnection::open_client`]. Each connection owns one publisher
/// and one subscriber on complementary iceoryx2 services.
///
/// CONSTRAINT: each `name` must be unique per logical connection. The
/// underlying iceoryx2 pub/sub is topic-based; multiple clients sharing
/// the same `name` will observe each other's messages.
///
/// WHY: no separate `IpcServer` struct — iceoryx2's `open_or_create`
/// is a single-step operation (no bind+accept), so a server-only
/// struct would be a single-use factory (litmus test #7).
pub struct IpcConnection {
    publisher: iceoryx2::port::publisher::Publisher<ipc_threadsafe::Service, IpcFrame, ()>,
    subscriber: iceoryx2::port::subscriber::Subscriber<ipc_threadsafe::Service, IpcFrame, ()>,
    // WHY: Node must be kept alive for the lifetime of the connection —
    // dropping it invalidates the iceoryx2 services.
    _node: iceoryx2::node::Node<ipc_threadsafe::Service>,
    closed: bool,
}

impl IpcConnection {
    /// Open the server side of an IPC channel.
    ///
    /// Creates a subscriber on `laic/ipc/{name}/c2s` (receives from
    /// clients) and a publisher on `laic/ipc/{name}/s2c` (sends to
    /// clients).
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::ConnectionFailed`] if iceoryx2 node
    /// or service creation fails.
    pub fn open_server(name: &str) -> Result<Self, LaicError> {
        // Server: subscribes to c2s, publishes on s2c
        Self::open(name, "c2s", "s2c")
    }

    /// Open the client side of an IPC channel.
    ///
    /// Creates a publisher on `laic/ipc/{name}/c2s` (sends to server)
    /// and a subscriber on `laic/ipc/{name}/s2c` (receives from server).
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::ConnectionFailed`] if iceoryx2 node
    /// or service creation fails.
    pub fn open_client(name: &str) -> Result<Self, LaicError> {
        // Client: publishes on c2s, subscribes to s2c
        Self::open(name, "s2c", "c2s")
    }

    /// Shared constructor: create a node, then open pub and sub services.
    ///
    /// `sub_dir` is the service suffix for the subscriber direction,
    /// `pub_dir` is the service suffix for the publisher direction.
    fn open(name: &str, sub_dir: &str, pub_dir: &str) -> Result<Self, LaicError> {
        let node = NodeBuilder::new()
            .create::<ipc_threadsafe::Service>()
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("failed to create iceoryx2 node: {e:?}"),
            })?;

        let sub_name = format!("laic/ipc/{name}/{sub_dir}");
        let pub_name = format!("laic/ipc/{name}/{pub_dir}");

        let sub_service_name: ServiceName =
            sub_name
                .as_str()
                .try_into()
                .map_err(|e| TransportError::ConnectionFailed {
                    detail: format!("invalid service name '{sub_name}': {e:?}"),
                })?;
        let pub_service_name: ServiceName =
            pub_name
                .as_str()
                .try_into()
                .map_err(|e| TransportError::ConnectionFailed {
                    detail: format!("invalid service name '{pub_name}': {e:?}"),
                })?;

        // CONSTRAINT: enable_safe_overflow(true) means subscriber buffer
        // full → oldest message silently replaced (lossy). Phase 4 flow
        // control is responsible for end-to-end reliability.
        let sub_service = node
            .service_builder(&sub_service_name)
            .publish_subscribe::<IpcFrame>()
            .subscriber_max_buffer_size(SUBSCRIBER_BUFFER_SIZE)
            .enable_safe_overflow(true)
            .open_or_create()
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("failed to open/create sub service '{sub_name}': {e:?}"),
            })?;

        let pub_service = node
            .service_builder(&pub_service_name)
            .publish_subscribe::<IpcFrame>()
            .subscriber_max_buffer_size(SUBSCRIBER_BUFFER_SIZE)
            .enable_safe_overflow(true)
            .open_or_create()
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("failed to open/create pub service '{pub_name}': {e:?}"),
            })?;

        let subscriber = sub_service.subscriber_builder().create().map_err(|e| {
            TransportError::ConnectionFailed {
                detail: format!("failed to create subscriber on '{sub_name}': {e:?}"),
            }
        })?;

        let publisher = pub_service.publisher_builder().create().map_err(|e| {
            TransportError::ConnectionFailed {
                detail: format!("failed to create publisher on '{pub_name}': {e:?}"),
            }
        })?;

        // WHY: explicitly discover peers so the first send/receive works
        // reliably. Without this, the publisher may not know about the
        // subscriber (or vice versa) and the first message can be lost.
        // WHY: errors discarded — update_connections() fails when no peer
        // exists yet (normal at creation time); the next send/receive will
        // re-discover automatically.
        let _: Result<(), _> = publisher.update_connections();
        let _: Result<(), _> = subscriber.update_connections();

        Ok(Self {
            publisher,
            subscriber,
            _node: node,
            closed: false,
        })
    }

    /// Send a LAIC message through shared memory.
    ///
    /// Encodes the message header into the IPC frame, copies the payload,
    /// and publishes via iceoryx2. A successful return means the message
    /// has been written to the SHM slot; it does **not** guarantee the
    /// subscriber has consumed it (see module-level delivery semantics).
    ///
    /// # Errors
    ///
    /// - [`TransportError::ShuttingDown`] if the connection is closed.
    /// - [`TransportError::FramingError`] if the payload exceeds
    ///   [`MAX_IPC_PAYLOAD_LEN`].
    /// - [`TransportError::BackpressureFull`] if the publisher's loan
    ///   limit is exceeded.
    /// - [`TransportError::SendFailed`] on other iceoryx2 errors.
    /// - [`LaicError::Protocol`] if the message header is invalid.
    // WHY: async to match enum Transport's async interface, even though
    // the send operation itself is synchronous (no await points).
    #[allow(clippy::unused_async)]
    pub async fn send(&mut self, msg: &Message) -> Result<(), LaicError> {
        if self.closed {
            return Err(TransportError::ShuttingDown.into());
        }

        let payload = msg.payload();
        if payload.len() > MAX_IPC_PAYLOAD_LEN as usize {
            return Err(TransportError::FramingError {
                detail: format!(
                    "payload length {} exceeds IPC maximum {MAX_IPC_PAYLOAD_LEN}",
                    payload.len()
                ),
            }
            .into());
        }

        // TRADEOFF: ~65KB IpcFrame on the stack, then copied to SHM slot.
        // Simple implementation; optimize with in-place writes if needed.
        #[allow(clippy::large_stack_arrays)]
        let mut frame = IpcFrame {
            header: [0u8; HEADER_SIZE],
            payload: [0u8; MAX_IPC_PAYLOAD_LEN as usize],
        };
        msg.header().encode(&mut frame.header)?;
        frame.payload[..payload.len()].copy_from_slice(payload);

        let sample = self.publisher.loan_uninit().map_err(|e| match e {
            iceoryx2::port::LoanError::ExceedsMaxLoans => TransportError::BackpressureFull,
            _ => TransportError::SendFailed {
                detail: format!("loan failed: {e:?}"),
            },
        })?;

        let sample = sample.write_payload(frame);
        sample.send().map_err(|e| TransportError::SendFailed {
            detail: format!("send failed: {e:?}"),
        })?;

        Ok(())
    }

    /// Receive the next LAIC message from shared memory.
    ///
    /// Polls the iceoryx2 subscriber and falls back to the default 100us
    /// sleep interval until a message arrives. A caller may explicitly opt
    /// into a bounded active-yield window with the
    /// `LAIC_IPC_ACTIVE_POLL_BUDGET_MS` environment variable.
    ///
    /// # Errors
    ///
    /// - [`TransportError::ShuttingDown`] if the connection is closed.
    /// - [`TransportError::ReceiveFailed`] on iceoryx2 receive errors.
    /// - [`LaicError::Protocol`] if the received header is invalid.
    pub async fn receive(&mut self) -> Result<Message, LaicError> {
        if self.closed {
            return Err(TransportError::ShuttingDown.into());
        }

        let active_poll_budget = active_poll_budget();
        let investigation_timing = std::env::var_os("LAIC_BHOST_IPC_RECEIVE_TIMING").is_some();
        let receive_start =
            (investigation_timing || active_poll_budget.is_some()).then(Instant::now);
        let active_poll_until =
            active_poll_budget.and_then(|budget| receive_start.map(|start| start + budget));
        let mut empty_polls = 0u64;
        let mut active_yields = 0u64;
        let mut requested_sleep = Duration::ZERO;
        let mut actual_sleep = Duration::ZERO;

        loop {
            match self.subscriber.receive() {
                Ok(Some(sample)) => {
                    if investigation_timing {
                        eprintln!(
                            "LAIC_BHOST_INVESTIGATION_IPC_RECEIVE empty_polls={empty_polls} active_yields={active_yields} requested_sleep_us={:.3} actual_sleep_us={:.3} elapsed_us={:.3}",
                            duration_us(requested_sleep),
                            duration_us(actual_sleep),
                            duration_us(receive_start.map_or(Duration::ZERO, |start| {
                                start.elapsed()
                            }))
                        );
                    }
                    let frame: &IpcFrame = &sample;
                    let header = MessageHeader::decode(&frame.header)?;
                    let payload_len = header.payload_len as usize;

                    // CONSTRAINT: IPC frame payload is fixed at
                    // MAX_IPC_PAYLOAD_LEN bytes. A decoded payload_len
                    // exceeding this limit means the frame is malformed
                    // (possibly from a cross-version peer or SHM
                    // corruption). Fail-closed instead of panicking on
                    // out-of-bounds slice.
                    if payload_len > MAX_IPC_PAYLOAD_LEN as usize {
                        return Err(TransportError::FramingError {
                            detail: format!(
                                "received payload_len {payload_len} exceeds \
                                 IPC frame capacity {MAX_IPC_PAYLOAD_LEN}"
                            ),
                        }
                        .into());
                    }

                    let payload = frame.payload[..payload_len].to_vec();
                    return Message::from_parts(header, payload);
                }
                Ok(None) => {
                    empty_polls += 1;
                    if active_poll_until.is_some_and(|deadline| Instant::now() < deadline) {
                        active_yields += 1;
                        // WHY: Tokio's task yield only cooperates with this
                        // runtime. Opted-in IPC benchmarks use cross-process
                        // peers, so also yield the OS thread before falling
                        // back to timer sleep.
                        std::thread::yield_now();
                        tokio::task::yield_now().await;
                        continue;
                    }
                    requested_sleep += POLL_INTERVAL;
                    let sleep_start = Instant::now();
                    tokio::time::sleep(POLL_INTERVAL).await;
                    actual_sleep += sleep_start.elapsed();
                }
                Err(e) => {
                    return Err(TransportError::ReceiveFailed {
                        detail: format!("receive failed: {e:?}"),
                    }
                    .into());
                }
            }
        }
    }

    /// Gracefully close this IPC connection.
    ///
    /// Sets the closed flag so subsequent `send` / `receive` calls
    /// return [`TransportError::ShuttingDown`]. The iceoryx2 publisher
    /// and subscriber resources are released when the `IpcConnection`
    /// is dropped.
    ///
    /// # Errors
    ///
    /// This method is infallible but returns `Result` to match the
    /// [`Transport`](super::Transport) enum interface.
    // WHY: async to match enum Transport's async interface, even though
    // the close operation is synchronous.
    #[allow(clippy::unused_async)]
    pub async fn close(&mut self) -> Result<(), LaicError> {
        self.closed = true;
        Ok(())
    }
}

fn active_poll_budget() -> Option<Duration> {
    *ACTIVE_POLL_BUDGET.get_or_init(|| {
        let raw = std::env::var(ACTIVE_POLL_BUDGET_MS_ENV).ok()?;
        let millis = raw.parse::<u64>().ok()?;
        (millis > 0).then(|| Duration::from_millis(millis))
    })
}

fn duration_us(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000_000.0
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::protocol::constants::{MAGIC, VERSION};

    /// Regression test for B1: a malformed IPC frame with `payload_len`
    /// exceeding `MAX_IPC_PAYLOAD_LEN` must return `FramingError`, not
    /// panic on out-of-bounds slice access.
    #[tokio::test]
    async fn receive_rejects_oversized_payload_len_in_header() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static CTR: AtomicU32 = AtomicU32::new(9000);
        let id = CTR.fetch_add(1, Ordering::Relaxed);
        let name = format!("b1test/{id}");

        let mut server = IpcConnection::open_server(&name).expect("open server");
        let client_conn = IpcConnection::open_client(&name).expect("open client");

        // Construct a valid-magic, valid-version header with
        // payload_len = MAX_IPC_PAYLOAD_LEN + 1 (exceeds frame capacity).
        #[allow(clippy::large_stack_arrays)]
        let mut frame = IpcFrame {
            header: [0u8; HEADER_SIZE],
            payload: [0u8; MAX_IPC_PAYLOAD_LEN as usize],
        };

        // Hand-encode header: magic(4) + version(2) + msg_type(2) +
        // msg_id(8) + correlation_id(8) + payload_len(4) + rest zeroed.
        frame.header[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        frame.header[4..6].copy_from_slice(&VERSION.to_le_bytes());
        frame.header[6..8].copy_from_slice(&1u16.to_le_bytes()); // msg_type = DATA
                                                                 // msg_id, correlation_id = 0 (already zeroed)
        let bad_len: u32 = MAX_IPC_PAYLOAD_LEN + 1;
        frame.header[24..28].copy_from_slice(&bad_len.to_le_bytes());
        frame.header[28] = 1; // payload_format = Protobuf
        frame.header[29] = 0; // qos = Normal
                              // reserved + checksum stay zero

        // Send the malicious frame directly via iceoryx2 publisher.
        let sample = client_conn.publisher.loan_uninit().expect("loan");
        let sample = sample.write_payload(frame);
        sample.send().expect("raw send");

        // Server receive must return FramingError, NOT panic.
        let err = server
            .receive()
            .await
            .expect_err("should reject oversized payload_len");
        assert_eq!(err.code().as_u16(), 0x0109); // FramingError
    }
}
