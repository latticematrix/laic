//! LAIC error types and error codes.
//!
//! Error hierarchy aligned with the logical architecture layers.
//! Each error carries a stable [`ErrorCode`] for cross-language SDK matching.

use core::fmt;

mod protocol;

pub use protocol::ProtocolError;

// ---------------------------------------------------------------------------
// ErrorCode
// ---------------------------------------------------------------------------

/// Stable numeric error code for cross-language SDK compatibility.
///
/// Encoding scheme:
/// - `0x01xx` — Transport errors
/// - `0x02xx` — Codec errors
/// - `0x03xx` — Protocol errors
/// - `0x04xx` — Flow control errors
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ErrorCode(u16);

impl ErrorCode {
    /// Returns the raw `u16` value.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:04X}", self.0)
    }
}

// ---------------------------------------------------------------------------
// LaicError
// ---------------------------------------------------------------------------

/// Top-level LAIC error.
///
/// Each variant maps to one logical architecture layer.
/// Marked `#[non_exhaustive]` so that new layers can be added in later
/// phases without breaking downstream crates.
#[non_exhaustive]
#[derive(Debug)]
pub enum LaicError {
    /// Protocol-level: invalid header fields, version mismatch, payload length.
    Protocol(ProtocolError),
    /// Codec-level: serialization / deserialization failure.
    Codec(CodecError),
    /// Transport-level: connection, send/receive, or framing failure.
    Transport(TransportError),
    /// Flow control: credit exhaustion.
    Flow(FlowError),
}

impl LaicError {
    /// Machine-readable error code for SDK matching.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Protocol(e) => e.code(),
            Self::Codec(e) => e.code(),
            Self::Transport(e) => e.code(),
            Self::Flow(e) => e.code(),
        }
    }

    /// Whether the caller should retry this operation.
    ///
    /// Follows **fail-closed** semantics: if uncertain, returns `false`.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        match self {
            Self::Protocol(e) => e.is_retryable(),
            Self::Codec(e) => e.is_retryable(),
            Self::Transport(e) => e.is_retryable(),
            Self::Flow(e) => e.is_retryable(),
        }
    }
}

impl fmt::Display for LaicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(e) => write!(f, "protocol error: {e}"),
            Self::Codec(e) => write!(f, "codec error: {e}"),
            Self::Transport(e) => write!(f, "transport error: {e}"),
            Self::Flow(e) => write!(f, "flow control error: {e}"),
        }
    }
}

impl std::error::Error for LaicError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Protocol(e) => Some(e),
            Self::Codec(e) => Some(e),
            Self::Transport(e) => Some(e),
            Self::Flow(e) => Some(e),
        }
    }
}

impl From<CodecError> for LaicError {
    fn from(e: CodecError) -> Self {
        Self::Codec(e)
    }
}

impl From<ProtocolError> for LaicError {
    fn from(e: ProtocolError) -> Self {
        Self::Protocol(e)
    }
}

impl From<TransportError> for LaicError {
    fn from(e: TransportError) -> Self {
        Self::Transport(e)
    }
}

impl From<FlowError> for LaicError {
    fn from(e: FlowError) -> Self {
        Self::Flow(e)
    }
}

// ---------------------------------------------------------------------------
// TransportError
// ---------------------------------------------------------------------------

/// Transport-level errors (error codes `0x01xx`).
///
/// Covers connection lifecycle, send/receive, and framing failures on
/// both IPC (iceoryx2) and QUIC (Quinn) transport backends.
///
/// Unlike protocol and codec errors, many transport errors are transient
/// and **retryable** (e.g. timeout, backpressure, connection lost).
#[non_exhaustive]
#[derive(Debug)]
pub enum TransportError {
    /// Connection or session could not be established.
    ConnectionFailed {
        /// Error detail from the transport backend.
        detail: String,
    },
    /// Connection was lost or reset by peer.
    ConnectionLost {
        /// Error detail from the transport backend.
        detail: String,
    },
    /// Transport is not in a connected state.
    NotConnected,
    /// Send operation failed.
    SendFailed {
        /// Error detail from the transport backend.
        detail: String,
    },
    /// Receive operation failed.
    ReceiveFailed {
        /// Error detail from the transport backend.
        detail: String,
    },
    /// Operation timed out.
    Timeout {
        /// Name of the operation that timed out.
        operation: String,
    },
    /// Transport backpressure: bounded send queue is full.
    ///
    /// TRADEOFF: transport-level backpressure only, not protocol-level
    /// flow control (Phase 4 credit-based system).
    BackpressureFull,
    /// Transport is shutting down; no new operations accepted.
    ShuttingDown,
    /// Framing error: message could not be read from or written to the wire.
    FramingError {
        /// Error detail describing the framing issue.
        detail: String,
    },
}

impl TransportError {
    /// Stable error code within the `0x01xx` range.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::ConnectionFailed { .. } => ErrorCode(0x0101),
            Self::ConnectionLost { .. } => ErrorCode(0x0102),
            Self::NotConnected => ErrorCode(0x0103),
            Self::SendFailed { .. } => ErrorCode(0x0104),
            Self::ReceiveFailed { .. } => ErrorCode(0x0105),
            Self::Timeout { .. } => ErrorCode(0x0106),
            Self::BackpressureFull => ErrorCode(0x0107),
            Self::ShuttingDown => ErrorCode(0x0108),
            Self::FramingError { .. } => ErrorCode(0x0109),
        }
    }

    /// Whether the caller should retry this operation.
    ///
    /// Transport errors follow **mixed retryability**: transient failures
    /// (timeout, backpressure, connection lost) are retryable, while
    /// permanent states (not connected, shutting down) and data
    /// corruption (framing error) are not.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        match self {
            Self::ConnectionFailed { .. }
            | Self::ConnectionLost { .. }
            | Self::SendFailed { .. }
            | Self::ReceiveFailed { .. }
            | Self::Timeout { .. }
            | Self::BackpressureFull => true,
            Self::NotConnected | Self::ShuttingDown | Self::FramingError { .. } => false,
        }
    }
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionFailed { detail } => write!(f, "connection failed: {detail}"),
            Self::ConnectionLost { detail } => write!(f, "connection lost: {detail}"),
            Self::NotConnected => write!(f, "not connected"),
            Self::SendFailed { detail } => write!(f, "send failed: {detail}"),
            Self::ReceiveFailed { detail } => write!(f, "receive failed: {detail}"),
            Self::Timeout { operation } => write!(f, "timeout: {operation}"),
            Self::BackpressureFull => write!(f, "backpressure: send queue full"),
            Self::ShuttingDown => write!(f, "transport shutting down"),
            Self::FramingError { detail } => write!(f, "framing error: {detail}"),
        }
    }
}

impl std::error::Error for TransportError {}

// ---------------------------------------------------------------------------
// CodecError
// ---------------------------------------------------------------------------

/// Codec-level errors (error codes `0x02xx`).
///
/// Wraps serialization / deserialization failures from the Arrow and
/// Protobuf codec backends.  The original error detail is captured as a
/// `String` to avoid leaking third-party types through the public API.
#[non_exhaustive]
#[derive(Debug)]
pub enum CodecError {
    /// Failed to serialize an Arrow `RecordBatch` to IPC bytes.
    ArrowEncode {
        /// Error detail from the Arrow library.
        detail: String,
    },
    /// Failed to deserialize Arrow IPC bytes to a `RecordBatch`.
    ArrowDecode {
        /// Error detail from the Arrow library.
        detail: String,
    },
    /// Failed to serialize a Protobuf message.
    ProtoEncode {
        /// Error detail from the prost library.
        detail: String,
    },
    /// Failed to deserialize Protobuf bytes to a message.
    ProtoDecode {
        /// Error detail from the prost library.
        detail: String,
    },
}

impl CodecError {
    /// Stable error code within the `0x02xx` range.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::ArrowEncode { .. } => ErrorCode(0x0201),
            Self::ArrowDecode { .. } => ErrorCode(0x0202),
            Self::ProtoEncode { .. } => ErrorCode(0x0203),
            Self::ProtoDecode { .. } => ErrorCode(0x0204),
        }
    }

    /// Whether the caller should retry.
    ///
    /// Codec errors are **never** retryable — the data is malformed
    /// and re-encoding / decoding the same input will produce the same
    /// error.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        // WHY: fail-closed — all codec errors are permanent.
        false
    }
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArrowEncode { detail } => write!(f, "Arrow encode failed: {detail}"),
            Self::ArrowDecode { detail } => write!(f, "Arrow decode failed: {detail}"),
            Self::ProtoEncode { detail } => write!(f, "Protobuf encode failed: {detail}"),
            Self::ProtoDecode { detail } => write!(f, "Protobuf decode failed: {detail}"),
        }
    }
}

impl std::error::Error for CodecError {}

// ---------------------------------------------------------------------------
// FlowError
// ---------------------------------------------------------------------------

/// Flow control errors (error codes `0x04xx`).
///
/// Covers credit-based flow control failures.
#[non_exhaustive]
#[derive(Debug)]
pub enum FlowError {
    /// Sender has no remaining credits; must wait for peer to replenish.
    ///
    /// Retryable: the caller should wait for a credit grant from the peer
    /// (piggybacked on the next received message) and retry.
    CreditExhausted,
}

impl FlowError {
    /// Stable error code within the `0x04xx` range.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::CreditExhausted => ErrorCode(0x0401),
        }
    }

    /// Whether the caller should retry this operation.
    ///
    /// Credit exhaustion is **retryable**: the caller should wait for a
    /// credit grant from the peer and retry.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        match self {
            Self::CreditExhausted => true,
        }
    }
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreditExhausted => write!(f, "credit exhausted: no credits remaining"),
        }
    }
}

impl std::error::Error for FlowError {}
