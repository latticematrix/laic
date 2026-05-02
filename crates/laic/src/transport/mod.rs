//! Transport layer: async message delivery over IPC and QUIC.
//!
//! # Lifecycle Contract
//!
//! Transport implementations follow a common lifecycle:
//!
//! - **Open**: create a transport instance and establish connectivity
//!   (IPC: create iceoryx2 services; QUIC: establish a connection with mTLS).
//! - **Send/Receive**: exchange [`super::protocol::message::Message`] frames.
//! - **Close**: graceful shutdown — mark the connection as closed so
//!   subsequent operations return
//!   [`TransportError::ShuttingDown`](crate::error::TransportError::ShuttingDown), then
//!   release resources. (QUIC finishes the stream; IPC sets a flag and
//!   defers SHM cleanup to drop.)
//! - **Drop**: if `close` was not called explicitly, resources are released
//!   on drop (best-effort, no ordering guarantee).
//!
//! Reconnect and backoff policies are **not** part of the transport contract
//! (deferred to Phase 4 / upper layers).
//!
//! # Framing (QUIC path)
//!
//! QUIC streams carry LAIC messages as sequential frames:
//!
//! ```text
//! [40-byte header][payload_len bytes of payload]
//! ```
//!
//! There is **no extra length prefix** — the header's `payload_len` field
//! is the single source of truth for payload size (Decision D8).
//!
//! The crate re-exports async read/write primitives for this wire format as
//! [`read_frame`] and [`write_frame`].

use crate::error::LaicError;
use crate::protocol::message::Message;

pub(crate) mod framing;
pub mod ipc;
pub mod quic;
pub mod tls;

pub use framing::{read_frame, write_frame, MAX_PAYLOAD_LEN};
pub use ipc::IpcConnection;
pub use quic::{QuicConnection, QuicServer};
pub use tls::{ClientTlsConfig, ServerTlsConfig};

// ---------------------------------------------------------------------------
// Transport enum — unified facade (Phase 3B-4)
// ---------------------------------------------------------------------------

/// Unified transport with compile-time known variants.
///
/// WHY: IPC and QUIC exhaust physical transport methods (SHM vs network).
/// Closed set — enum is more honest than open trait for two known variants.
///
/// Construction is backend-specific: use [`IpcConnection::open_server`] /
/// [`IpcConnection::open_client`] or [`QuicConnection::connect`], then
/// wrap in the appropriate variant.
///
/// # Example
///
/// ```ignore
/// let mut transport = Transport::Ipc(IpcConnection::open_client("my-channel")?);
/// transport.send(&msg).await?;
/// ```
pub enum Transport {
    /// Local shared-memory transport via iceoryx2.
    Ipc(IpcConnection),
    /// Remote encrypted transport via Quinn/QUIC.
    Quic(QuicConnection),
}

impl Transport {
    /// Send a message through the underlying transport backend.
    ///
    /// Delegates to [`IpcConnection::send`] or [`QuicConnection::send`]
    /// depending on the variant.
    ///
    /// # Errors
    ///
    /// Returns the same errors as the underlying backend's `send` method.
    pub async fn send(&mut self, msg: &Message) -> Result<(), LaicError> {
        match self {
            Self::Ipc(c) => c.send(msg).await,
            Self::Quic(c) => c.send(msg).await,
        }
    }

    /// Receive the next message from the underlying transport backend.
    ///
    /// Delegates to [`IpcConnection::receive`] or
    /// [`QuicConnection::receive`] depending on the variant.
    ///
    /// # Errors
    ///
    /// Returns the same errors as the underlying backend's `receive` method.
    pub async fn receive(&mut self) -> Result<Message, LaicError> {
        match self {
            Self::Ipc(c) => c.receive().await,
            Self::Quic(c) => c.receive().await,
        }
    }

    /// Gracefully close the underlying transport backend.
    ///
    /// Delegates to [`IpcConnection::close`] or [`QuicConnection::close`]
    /// depending on the variant.
    ///
    /// # Errors
    ///
    /// Returns the same errors as the underlying backend's `close` method.
    pub async fn close(&mut self) -> Result<(), LaicError> {
        match self {
            Self::Ipc(c) => c.close().await,
            Self::Quic(c) => c.close().await,
        }
    }
}
