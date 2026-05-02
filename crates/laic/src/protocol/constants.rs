//! Protocol constants, type codes, and flag definitions.

use crate::error::ProtocolError;

// ---------------------------------------------------------------------------
// Magic & version
// ---------------------------------------------------------------------------

/// LAIC header magic number: `0x4C414943` ("LAIC" in ASCII).
pub const MAGIC: u32 = 0x4C41_4943;

/// Current protocol version: major 0, minor 1.
///
/// Encoding: `major << 8 | minor`.
pub const VERSION: u16 = 0x0001;

/// Fixed header size in bytes.
pub const HEADER_SIZE: usize = 40;

// ---------------------------------------------------------------------------
// PayloadFormat
// ---------------------------------------------------------------------------

/// Payload serialisation format.
///
/// Maps to the `payload_format` byte in [`super::header::MessageHeader`].
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PayloadFormat {
    /// Apache Arrow IPC — tensor / embedding data plane.
    Arrow = 0,
    /// Protocol Buffers — control messages and remote calls.
    Protobuf = 1,
    /// Raw bytes — opaque payload, no codec interpretation.
    Raw = 2,
}

impl PayloadFormat {
    /// Convert from raw byte, returning an error for unknown values.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::InvalidPayloadFormat`] if `value` is not
    /// a known format.
    pub const fn from_u8(value: u8) -> Result<Self, ProtocolError> {
        match value {
            0 => Ok(Self::Arrow),
            1 => Ok(Self::Protobuf),
            2 => Ok(Self::Raw),
            _ => Err(ProtocolError::InvalidPayloadFormat { value }),
        }
    }
}

// ---------------------------------------------------------------------------
// Qos
// ---------------------------------------------------------------------------

/// Quality-of-service priority level.
///
/// Maps to the `qos` byte in [`super::header::MessageHeader`].
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Qos {
    /// Normal priority — default for data messages.
    Normal = 0,
    /// High priority — expedited delivery.
    High = 1,
    /// Emergency priority — LP7 emergency channel.
    Emergency = 2,
}

impl Qos {
    /// Convert from raw byte, returning an error for unknown values.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::InvalidQos`] if `value` is not a known level.
    pub const fn from_u8(value: u8) -> Result<Self, ProtocolError> {
        match value {
            0 => Ok(Self::Normal),
            1 => Ok(Self::High),
            2 => Ok(Self::Emergency),
            _ => Err(ProtocolError::InvalidQos { value }),
        }
    }
}

// ---------------------------------------------------------------------------
// MsgType
// ---------------------------------------------------------------------------

/// Message type identifier.
///
/// Uses a newtype over `u16` rather than an enum because the set is
/// **open**: the Pattern layer (Phase 5) will define additional types
/// without modifying this module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MsgType(u16);

impl MsgType {
    // Core types (0x00xx) — defined here.

    /// Generic data message.
    pub const DATA: Self = Self(0x0001);
    /// Control message (handshake, heartbeat, etc.).
    pub const CONTROL: Self = Self(0x0002);
    /// Acknowledgement.
    pub const ACK: Self = Self(0x0003);
    /// Keepalive heartbeat.
    pub const HEARTBEAT: Self = Self(0x0004);

    /// Create from a raw `u16` value.
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Return the raw `u16` value.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// Header flags (bit positions)
// ---------------------------------------------------------------------------

/// Flag bit: payload is compressed.
pub const FLAG_COMPRESSED: u32 = 1 << 0;

/// Flag bit: message is a fragment of a larger message.
pub const FLAG_FRAGMENTED: u32 = 1 << 1;

/// Flag bit: sender requests an acknowledgement.
pub const FLAG_ACK_REQUESTED: u32 = 1 << 2;

/// Flag bit: marks the final chunk of a stream.
pub const FLAG_END_OF_STREAM: u32 = 1 << 3;

/// Flag bit: `credit_grant` field carries a non-zero value.
pub const FLAG_HAS_CREDIT_GRANT: u32 = 1 << 4;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_is_laic_ascii() {
        assert_eq!(&MAGIC.to_be_bytes(), b"LAIC");
    }

    #[test]
    fn version_encoding() {
        let major = VERSION >> 8;
        let minor = VERSION & 0xFF;
        assert_eq!(major, 0);
        assert_eq!(minor, 1);
    }

    #[test]
    fn payload_format_roundtrip() {
        for &fmt in &[
            PayloadFormat::Arrow,
            PayloadFormat::Protobuf,
            PayloadFormat::Raw,
        ] {
            let byte = fmt as u8;
            let back = PayloadFormat::from_u8(byte);
            assert!(back.is_ok());
        }
    }

    #[test]
    fn payload_format_invalid() {
        assert!(PayloadFormat::from_u8(3).is_err());
        assert!(PayloadFormat::from_u8(255).is_err());
    }

    #[test]
    fn qos_roundtrip() {
        for &q in &[Qos::Normal, Qos::High, Qos::Emergency] {
            let byte = q as u8;
            let back = Qos::from_u8(byte);
            assert!(back.is_ok());
        }
    }

    #[test]
    fn qos_invalid() {
        assert!(Qos::from_u8(3).is_err());
        assert!(Qos::from_u8(255).is_err());
    }

    #[test]
    fn msg_type_constants() {
        assert_eq!(MsgType::DATA.as_u16(), 0x0001);
        assert_eq!(MsgType::CONTROL.as_u16(), 0x0002);
        assert_eq!(MsgType::ACK.as_u16(), 0x0003);
        assert_eq!(MsgType::HEARTBEAT.as_u16(), 0x0004);
    }

    #[test]
    fn flags_are_distinct_bits() {
        let all = [
            FLAG_COMPRESSED,
            FLAG_FRAGMENTED,
            FLAG_ACK_REQUESTED,
            FLAG_END_OF_STREAM,
            FLAG_HAS_CREDIT_GRANT,
        ];
        // No overlap.
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_eq!(
                    all[i] & all[j],
                    0,
                    "flag overlap: {:#X} & {:#X}",
                    all[i],
                    all[j]
                );
            }
        }
        // Each is a single bit.
        for &flag in &all {
            assert_eq!(flag.count_ones(), 1, "not a single bit: {flag:#X}");
        }
    }
}
