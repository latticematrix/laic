//! Emergency channel: physically isolated SHM transport for LP7 raw bytes.
//!
//! Provides a dedicated shared-memory channel for LP7 emergency messages,
//! separate from the main IPC transport. Transmits raw byte payloads
//! **without** LAIC header wrapping — the data plane is LP7-owned.
//!
//! # Delivery Semantics
//!
//! This is a **best-effort / lossy** channel. When the subscriber buffer
//! is full, the oldest unread message is silently replaced by the newest
//! one (`enable_safe_overflow`). `send()` success means the payload was
//! written to a SHM slot — it does **not** guarantee the receiver will
//! read it before it is overwritten.
//!
//! WHY: physical isolation ensures emergency messages cannot be blocked
//! by backpressure on the main transport path. Different API surface
//! (`&[u8]` / `Vec<u8>`) from [`crate::transport::ipc::IpcConnection`]
//! (`&Message` / `Message`) — does not fit the `Transport` enum.

use std::time::Duration;

use iceoryx2::port::update_connections::UpdateConnections;
use iceoryx2::prelude::*;

// WHY: reuses TransportError — emergency channel operations are transport
// operations (connection, send, receive, framing), not a distinct error domain.
use crate::error::{LaicError, TransportError};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum emergency payload length (168 bytes).
///
/// CONSTRAINT: LP7 emergency messages are small fixed-size alerts.
/// 168 bytes covers LP7 header (120B) + heartbeat fields (32B) +
/// future extension margin (16B).
pub const MAX_EMERGENCY_PAYLOAD_LEN: usize = 168;

/// Total emergency frame size (4-byte length prefix + payload buffer).
///
/// WHY: expression instead of literal 172 — stays correct if
/// `MAX_EMERGENCY_PAYLOAD_LEN` changes.
const EMERGENCY_FRAME_SIZE: usize = 4 + MAX_EMERGENCY_PAYLOAD_LEN;

/// Polling interval for the emergency receive loop.
///
/// TRADEOFF: 50μs vs 100μs (IPC). Emergency messages are time-critical;
/// halving the poll interval doubles worst-case latency improvement at
/// negligible CPU cost for a single lightweight channel.
const EMERGENCY_POLL_INTERVAL: Duration = Duration::from_micros(50);

/// Subscriber buffer depth for the emergency channel.
///
/// WHY: smaller than IPC (16) because emergency messages are rare bursts.
/// 8 slots × 172B ≈ 1.4 KB — negligible memory footprint.
const EMERGENCY_BUFFER_SIZE: usize = 8;

// ---------------------------------------------------------------------------
// EmergencyFrame — SHM transmission unit
// ---------------------------------------------------------------------------

/// SHM transport frame for emergency messages: 4-byte length + payload.
///
/// WHY: separate from `IpcFrame` because the payload size, frame layout,
/// and API semantics are entirely different. Sharing an abstraction would
/// be forced (litmus test #2/#3).
#[derive(Clone, Copy, ZeroCopySend)]
#[repr(C)]
pub(crate) struct EmergencyFrame {
    /// Actual payload length (up to `MAX_EMERGENCY_PAYLOAD_LEN`).
    len: u32,
    /// Payload buffer — only `len` bytes are valid.
    data: [u8; MAX_EMERGENCY_PAYLOAD_LEN],
}

/// Compile-time guarantee: frame is exactly 172 bytes.
const _: () = assert!(core::mem::size_of::<EmergencyFrame>() == EMERGENCY_FRAME_SIZE);

impl core::fmt::Debug for EmergencyFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EmergencyFrame")
            .field("len", &self.len)
            .field("data", &format_args!("[u8; {}]", self.data.len()))
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EmergencyChannel
// ---------------------------------------------------------------------------

/// Physically isolated SHM channel for LP7 emergency messages.
///
/// Operates independently from the main IPC transport. Transmits raw
/// bytes without LAIC header wrapping — LP7 owns the payload format.
///
/// WHY: not in the `Transport` enum — API is `&[u8]` / `Vec<u8>`
/// instead of `&Message` / `Message`, and emergency messages bypass
/// the LAIC protocol stack entirely.
///
/// CONSTRAINT: each `name` identifies a 1:1 channel (same as IPC).
pub struct EmergencyChannel {
    publisher: iceoryx2::port::publisher::Publisher<ipc_threadsafe::Service, EmergencyFrame, ()>,
    subscriber: iceoryx2::port::subscriber::Subscriber<ipc_threadsafe::Service, EmergencyFrame, ()>,
    // WHY: Node must be kept alive — dropping it invalidates services.
    _node: iceoryx2::node::Node<ipc_threadsafe::Service>,
    closed: bool,
}

impl EmergencyChannel {
    /// Open the server side of an emergency channel.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::ConnectionFailed`] if iceoryx2 setup fails.
    pub fn open_server(name: &str) -> Result<Self, LaicError> {
        Self::open(name, "c2s", "s2c")
    }

    /// Open the client side of an emergency channel.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::ConnectionFailed`] if iceoryx2 setup fails.
    pub fn open_client(name: &str) -> Result<Self, LaicError> {
        Self::open(name, "s2c", "c2s")
    }

    /// Shared constructor: create a node, then open pub and sub services.
    fn open(name: &str, sub_dir: &str, pub_dir: &str) -> Result<Self, LaicError> {
        let node = NodeBuilder::new()
            .create::<ipc_threadsafe::Service>()
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("emergency node creation failed: {e:?}"),
            })?;

        let sub_name = format!("laic/emergency/{name}/{sub_dir}");
        let pub_name = format!("laic/emergency/{name}/{pub_dir}");

        let sub_svc_name: ServiceName =
            sub_name
                .as_str()
                .try_into()
                .map_err(|e| TransportError::ConnectionFailed {
                    detail: format!("invalid service name '{sub_name}': {e:?}"),
                })?;
        let pub_svc_name: ServiceName =
            pub_name
                .as_str()
                .try_into()
                .map_err(|e| TransportError::ConnectionFailed {
                    detail: format!("invalid service name '{pub_name}': {e:?}"),
                })?;

        let sub_service = node
            .service_builder(&sub_svc_name)
            .publish_subscribe::<EmergencyFrame>()
            .subscriber_max_buffer_size(EMERGENCY_BUFFER_SIZE)
            .enable_safe_overflow(true)
            .open_or_create()
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("emergency sub service '{sub_name}' failed: {e:?}"),
            })?;

        let pub_service = node
            .service_builder(&pub_svc_name)
            .publish_subscribe::<EmergencyFrame>()
            .subscriber_max_buffer_size(EMERGENCY_BUFFER_SIZE)
            .enable_safe_overflow(true)
            .open_or_create()
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("emergency pub service '{pub_name}' failed: {e:?}"),
            })?;

        let subscriber = sub_service.subscriber_builder().create().map_err(|e| {
            TransportError::ConnectionFailed {
                detail: format!("emergency subscriber on '{sub_name}' failed: {e:?}"),
            }
        })?;

        let publisher = pub_service.publisher_builder().create().map_err(|e| {
            TransportError::ConnectionFailed {
                detail: format!("emergency publisher on '{pub_name}' failed: {e:?}"),
            }
        })?;

        // WHY: explicitly discover peers for reliable first message delivery.
        // Errors discarded — normal when no peer exists yet.
        let _: Result<(), _> = publisher.update_connections();
        let _: Result<(), _> = subscriber.update_connections();

        Ok(Self {
            publisher,
            subscriber,
            _node: node,
            closed: false,
        })
    }

    /// Send raw bytes through the emergency channel.
    ///
    /// CONSTRAINT: best-effort / lossy — success means the payload was
    /// written to a SHM slot. If the subscriber buffer is full, the
    /// oldest unread message is silently replaced. There is no
    /// guaranteed-delivery guarantee.
    ///
    /// # Errors
    ///
    /// - [`TransportError::ShuttingDown`] if the channel is closed.
    /// - [`TransportError::FramingError`] if `payload` exceeds
    ///   [`MAX_EMERGENCY_PAYLOAD_LEN`].
    /// - [`TransportError::BackpressureFull`] if the loan limit is reached.
    /// - [`TransportError::SendFailed`] on other iceoryx2 errors.
    #[allow(clippy::unused_async)]
    pub async fn send(&mut self, payload: &[u8]) -> Result<(), LaicError> {
        if self.closed {
            return Err(TransportError::ShuttingDown.into());
        }
        if payload.len() > MAX_EMERGENCY_PAYLOAD_LEN {
            return Err(TransportError::FramingError {
                detail: format!(
                    "emergency payload length {} exceeds maximum {MAX_EMERGENCY_PAYLOAD_LEN}",
                    payload.len()
                ),
            }
            .into());
        }

        let mut frame = EmergencyFrame {
            len: 0,
            data: [0u8; MAX_EMERGENCY_PAYLOAD_LEN],
        };
        #[allow(clippy::cast_possible_truncation)]
        {
            frame.len = payload.len() as u32;
        }
        frame.data[..payload.len()].copy_from_slice(payload);

        let sample = self.publisher.loan_uninit().map_err(|e| match e {
            iceoryx2::port::LoanError::ExceedsMaxLoans => TransportError::BackpressureFull,
            _ => TransportError::SendFailed {
                detail: format!("emergency loan failed: {e:?}"),
            },
        })?;
        let sample = sample.write_payload(frame);
        sample.send().map_err(|e| TransportError::SendFailed {
            detail: format!("emergency send failed: {e:?}"),
        })?;

        Ok(())
    }

    /// Receive the next emergency payload as raw bytes.
    ///
    /// Polls with the emergency channel's 50μs interval until data arrives.
    ///
    /// # Errors
    ///
    /// - [`TransportError::ShuttingDown`] if the channel is closed.
    /// - [`TransportError::FramingError`] if the frame `len` exceeds capacity.
    /// - [`TransportError::ReceiveFailed`] on iceoryx2 errors.
    pub async fn receive(&mut self) -> Result<Vec<u8>, LaicError> {
        if self.closed {
            return Err(TransportError::ShuttingDown.into());
        }

        loop {
            match self.subscriber.receive() {
                Ok(Some(sample)) => {
                    let frame: &EmergencyFrame = &sample;
                    let len = frame.len as usize;

                    // CONSTRAINT: fail-closed on malformed frame — a len
                    // exceeding MAX_EMERGENCY_PAYLOAD_LEN means SHM
                    // corruption or cross-version peer.
                    if len > MAX_EMERGENCY_PAYLOAD_LEN {
                        return Err(TransportError::FramingError {
                            detail: format!(
                                "emergency frame len {len} exceeds capacity \
                                 {MAX_EMERGENCY_PAYLOAD_LEN}"
                            ),
                        }
                        .into());
                    }

                    return Ok(frame.data[..len].to_vec());
                }
                Ok(None) => {
                    tokio::time::sleep(EMERGENCY_POLL_INTERVAL).await;
                }
                Err(e) => {
                    return Err(TransportError::ReceiveFailed {
                        detail: format!("emergency receive failed: {e:?}"),
                    }
                    .into());
                }
            }
        }
    }

    /// Gracefully close this emergency channel.
    ///
    /// # Errors
    ///
    /// Infallible; returns `Result` for API consistency.
    #[allow(clippy::unused_async)]
    pub async fn close(&mut self) -> Result<(), LaicError> {
        self.closed = true;
        Ok(())
    }
}
