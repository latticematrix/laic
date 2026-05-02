use core::fmt;

use super::ErrorCode;

/// Protocol-level errors (error codes `0x03xx`).
#[non_exhaustive]
#[derive(Debug)]
pub enum ProtocolError {
    /// Header magic number does not match `0x4C414943`.
    InvalidMagic {
        /// The incorrect magic value that was found.
        actual: u32,
    },
    /// Buffer is too short to contain a full header.
    BufferTooShort {
        /// Number of bytes provided.
        actual: usize,
        /// Number of bytes required.
        expected: usize,
    },
    /// Protocol version is not supported.
    UnsupportedVersion {
        /// The unsupported version value.
        version: u16,
    },
    /// Payload format byte is out of range.
    InvalidPayloadFormat {
        /// The invalid format byte.
        value: u8,
    },
    /// `QoS` byte is out of range.
    InvalidQos {
        /// The invalid `QoS` byte.
        value: u8,
    },
    /// Header `payload_len` does not match actual payload length.
    PayloadLengthMismatch {
        /// Length declared in header.
        header_len: u32,
        /// Actual payload buffer length.
        actual_len: usize,
    },
    /// Message type does not match the expected type for this operation.
    UnexpectedMessageType {
        /// The expected message type.
        expected: u16,
        /// The actual message type found.
        actual: u16,
    },
    /// Payload format does not match the expected contract for this operation.
    UnexpectedPayloadFormat {
        /// The payload format this operation requires.
        expected: u8,
        /// The payload format actually found on the wire.
        actual: u8,
    },
    /// Trust-domain handshake version does not match the local version.
    UnsupportedHandshakeVersion {
        /// The version this endpoint requires.
        expected: u16,
        /// The version presented by the peer.
        actual: u16,
    },
    /// Trust-domain handshake reported a remote domain that does not match
    /// the locally expected peer domain.
    TrustDomainMismatch {
        /// The domain name this endpoint expected from the peer.
        expected: String,
        /// The domain name actually reported by the peer.
        actual: String,
    },
    /// Trust-domain handshake did not echo the client nonce correctly.
    HandshakeNonceMismatch,
    /// Trust-domain handshake payload is structurally malformed.
    InvalidHandshakePayload {
        /// Human-readable detail naming the malformed field.
        detail: String,
    },
}

impl ProtocolError {
    /// Stable error code within the `0x03xx` range.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidMagic { .. } => ErrorCode(0x0301),
            Self::BufferTooShort { .. } => ErrorCode(0x0302),
            Self::UnsupportedVersion { .. } => ErrorCode(0x0303),
            Self::InvalidPayloadFormat { .. } => ErrorCode(0x0304),
            Self::InvalidQos { .. } => ErrorCode(0x0305),
            Self::PayloadLengthMismatch { .. } => ErrorCode(0x0306),
            Self::UnexpectedMessageType { .. } => ErrorCode(0x0307),
            // WHY: these handshake-oriented codes were already exported before
            // `UnexpectedPayloadFormat` was added. Keep their numeric values
            // stable for cross-language SDK matching and append the new code
            // at the next free slot instead of renumbering the existing range.
            Self::UnsupportedHandshakeVersion { .. } => ErrorCode(0x0308),
            Self::TrustDomainMismatch { .. } => ErrorCode(0x0309),
            Self::HandshakeNonceMismatch => ErrorCode(0x030A),
            Self::InvalidHandshakePayload { .. } => ErrorCode(0x030B),
            Self::UnexpectedPayloadFormat { .. } => ErrorCode(0x030C),
        }
    }

    /// Whether the caller should retry.
    ///
    /// Protocol errors are **never** retryable — the message itself is
    /// malformed and resending the same bytes will produce the same error.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        // WHY: fail-closed — all protocol errors are permanent.
        false
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic { actual } => {
                write!(f, "invalid magic: expected 0x4C414943, got 0x{actual:08X}")
            }
            Self::BufferTooShort { actual, expected } => {
                write!(f, "buffer too short: {actual} bytes, need {expected}")
            }
            Self::UnsupportedVersion { version } => {
                write!(f, "unsupported version: 0x{version:04X}")
            }
            Self::InvalidPayloadFormat { value } => {
                write!(f, "invalid payload format: {value}")
            }
            Self::InvalidQos { value } => {
                write!(f, "invalid QoS: {value}")
            }
            Self::PayloadLengthMismatch {
                header_len,
                actual_len,
            } => {
                write!(
                    f,
                    "payload length mismatch: header says {header_len}, actual {actual_len}"
                )
            }
            Self::UnexpectedMessageType { expected, actual } => {
                write!(
                    f,
                    "unexpected message type: expected 0x{expected:04X}, got 0x{actual:04X}"
                )
            }
            Self::UnexpectedPayloadFormat { expected, actual } => {
                write!(
                    f,
                    "unexpected payload format: expected 0x{expected:02X}, got 0x{actual:02X}"
                )
            }
            Self::UnsupportedHandshakeVersion { expected, actual } => {
                write!(
                    f,
                    "unsupported handshake version: expected 0x{expected:04X}, got 0x{actual:04X}"
                )
            }
            Self::TrustDomainMismatch { expected, actual } => {
                write!(
                    f,
                    "trust-domain mismatch: expected {expected}, got {actual}"
                )
            }
            Self::HandshakeNonceMismatch => {
                write!(f, "handshake nonce mismatch")
            }
            Self::InvalidHandshakePayload { detail } => {
                write!(f, "invalid handshake payload: {detail}")
            }
        }
    }
}

impl std::error::Error for ProtocolError {}
