//! IPC ↔ QUIC protocol bridge.
//!
//! [`Gateway`] uses two transport instances ([`IpcConnection`] +
//! [`QuicConnection`]) to forward messages across the local/remote
//! boundary. It is **not** a third `Transport` variant — it is an
//! assembler that delegates to existing backends.
//!
//! # Design Constraints
//!
//! - **Single-message, single-direction**: each `forward_*` call moves
//!   exactly one message. Bidirectional forwarding loops are the
//!   caller's responsibility (Mechanism, not Policy).
//! - **Best-effort forwarding**: if `receive` succeeds but `send` fails,
//!   the message is lost. LAIC provides "network wire" semantics, not
//!   persistent messaging.

use crate::error::LaicError;
use crate::transport::ipc::IpcConnection;
use crate::transport::quic::QuicConnection;

// ---------------------------------------------------------------------------
// Gateway
// ---------------------------------------------------------------------------

/// IPC ↔ QUIC protocol bridge.
///
/// WHY: not a third Transport variant — Gateway is an assembler that
/// uses two Transport instances, not an independent transport backend.
///
/// CONSTRAINT: forward methods are single-message, single-direction.
/// Bidirectional forwarding loops are caller's responsibility (Mechanism
/// not Policy — the caller decides forwarding strategy).
pub struct Gateway {
    ipc: IpcConnection,
    quic: QuicConnection,
}

impl Gateway {
    /// Create a new gateway bridging an IPC and a QUIC connection.
    #[must_use]
    pub fn new(ipc: IpcConnection, quic: QuicConnection) -> Self {
        Self { ipc, quic }
    }

    /// Forward one message: IPC → QUIC.
    ///
    /// Receives from the IPC side and sends over QUIC.
    ///
    /// CONSTRAINT: best-effort — if `receive` succeeds but `send` fails,
    /// the message is lost ("network wire" semantics).
    ///
    /// # Errors
    ///
    /// Returns the first error encountered during receive or send.
    pub async fn forward_ipc_to_quic(&mut self) -> Result<(), LaicError> {
        let msg = self.ipc.receive().await?;
        self.quic.send(&msg).await
    }

    /// Forward one message: QUIC → IPC.
    ///
    /// Receives from the QUIC side and sends over IPC.
    ///
    /// CONSTRAINT: best-effort — if `receive` succeeds but `send` fails,
    /// the message is lost ("network wire" semantics).
    ///
    /// # Errors
    ///
    /// Returns the first error encountered during receive or send.
    pub async fn forward_quic_to_ipc(&mut self) -> Result<(), LaicError> {
        let msg = self.quic.receive().await?;
        self.ipc.send(&msg).await
    }

    /// Close both transports.
    ///
    /// Always attempts to close both sides, even if the first one fails.
    /// Returns the first error encountered (if any).
    ///
    /// CONSTRAINT: closes IPC first, then QUIC. Both are always
    /// attempted — the second close is not skipped if the first fails.
    ///
    /// # Errors
    ///
    /// Returns the first error from either `close` call.
    pub async fn close(&mut self) -> Result<(), LaicError> {
        let ipc_result = self.ipc.close().await;
        let quic_result = self.quic.close().await;
        ipc_result.and(quic_result)
    }

    /// Decompose into parts for caller-managed bidirectional loops.
    ///
    /// Useful when the caller needs direct access to both connections
    /// for custom forwarding strategies.
    #[must_use]
    pub fn into_parts(self) -> (IpcConnection, QuicConnection) {
        (self.ipc, self.quic)
    }
}
