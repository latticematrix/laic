//! LAIC message header: fixed 40-byte `repr(C)` layout.

use crate::error::{LaicError, ProtocolError};
use crate::protocol::constants::{PayloadFormat, Qos, HEADER_SIZE, MAGIC, VERSION};

// ---------------------------------------------------------------------------
// Compile-time layout verification
// ---------------------------------------------------------------------------

/// Compile-time guarantee: header is exactly 40 bytes.
const _: () = assert!(core::mem::size_of::<MessageHeader>() == HEADER_SIZE);

/// Compile-time guarantee: struct alignment is 8 (due to u64 fields).
const _: () = assert!(core::mem::align_of::<MessageHeader>() == 8);

// ---------------------------------------------------------------------------
// MessageHeader
// ---------------------------------------------------------------------------

/// Fixed-layout LAIC message header for zero-copy SHM access.
///
/// Total size: 40 bytes, naturally aligned under `repr(C)`.
/// All multi-byte fields use **little-endian** encoding on the wire.
///
/// # Wire layout
///
/// | Offset | Field            | Type      |
/// |--------|------------------|-----------|
/// |  0     | `magic`          | `u32`     |
/// |  4     | `version`        | `u16`     |
/// |  6     | `msg_type`       | `u16`     |
/// |  8     | `msg_id`         | `u64`     |
/// | 16     | `correlation_id` | `u64`     |
/// | 24     | `payload_len`    | `u32`     |
/// | 28     | `payload_format` | `u8`      |
/// | 29     | `qos`            | `u8`      |
/// | 30     | `credit_grant`   | `u16`     |
/// | 32     | `flags`          | `u32`     |
/// | 36     | `reserved`       | `[u8; 4]` |
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MessageHeader {
    /// Magic number: `0x4C414943` ("LAIC" in ASCII).
    pub magic: u32,
    /// Protocol version: `major << 8 | minor`.
    pub version: u16,
    /// Message type code (see [`super::constants::MsgType`]).
    pub msg_type: u16,
    /// Unique message identifier.
    pub msg_id: u64,
    /// Correlation ID: links a response to its request.
    pub correlation_id: u64,
    /// Total payload length in bytes.
    pub payload_len: u32,
    /// Payload format (see [`PayloadFormat`]).
    pub payload_format: u8,
    /// `QoS` priority level (see [`Qos`]).
    pub qos: u8,
    /// Flow control: number of credits granted to peer.
    pub credit_grant: u16,
    /// General-purpose bit flags (transport / frame level only).
    pub flags: u32,
    /// Reserved for future extension.
    pub reserved: [u8; 4],
}

impl MessageHeader {
    /// Encode this header into `buf` using little-endian byte order.
    ///
    /// Validates all protocol-invariant fields (`magic`, `version`,
    /// `payload_format`, `qos`) before serializing — the sender must not
    /// emit bytes that the receiver would reject.
    ///
    /// # Errors
    ///
    /// Returns [`LaicError::Protocol`] if `buf` is shorter than
    /// [`HEADER_SIZE`] bytes, or if any protocol-invariant field contains
    /// an invalid value.
    pub fn encode(&self, buf: &mut [u8]) -> Result<(), LaicError> {
        if buf.len() < HEADER_SIZE {
            return Err(ProtocolError::BufferTooShort {
                actual: buf.len(),
                expected: HEADER_SIZE,
            }
            .into());
        }

        // WHY: validate all protocol-invariant fields at the sender
        // boundary — do not emit bytes that our own decoder would reject,
        // even if the struct was constructed with raw field values.
        if self.magic != MAGIC {
            return Err(ProtocolError::InvalidMagic { actual: self.magic }.into());
        }
        if self.version != VERSION {
            return Err(ProtocolError::UnsupportedVersion {
                version: self.version,
            }
            .into());
        }
        PayloadFormat::from_u8(self.payload_format)?;
        Qos::from_u8(self.qos)?;

        buf[0..4].copy_from_slice(&self.magic.to_le_bytes());
        buf[4..6].copy_from_slice(&self.version.to_le_bytes());
        buf[6..8].copy_from_slice(&self.msg_type.to_le_bytes());
        buf[8..16].copy_from_slice(&self.msg_id.to_le_bytes());
        buf[16..24].copy_from_slice(&self.correlation_id.to_le_bytes());
        buf[24..28].copy_from_slice(&self.payload_len.to_le_bytes());
        buf[28] = self.payload_format;
        buf[29] = self.qos;
        buf[30..32].copy_from_slice(&self.credit_grant.to_le_bytes());
        buf[32..36].copy_from_slice(&self.flags.to_le_bytes());
        buf[36..40].copy_from_slice(&self.reserved);

        Ok(())
    }

    /// Decode a header from `buf`, validating magic, version, format,
    /// and `QoS`.
    ///
    /// # Errors
    ///
    /// Returns [`LaicError::Protocol`] if the buffer is too short, the
    /// magic is wrong, the version is unsupported, or any closed-set
    /// field value is out of range.
    pub fn decode(buf: &[u8]) -> Result<Self, LaicError> {
        if buf.len() < HEADER_SIZE {
            return Err(ProtocolError::BufferTooShort {
                actual: buf.len(),
                expected: HEADER_SIZE,
            }
            .into());
        }

        let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        if magic != MAGIC {
            return Err(ProtocolError::InvalidMagic { actual: magic }.into());
        }

        let version = u16::from_le_bytes([buf[4], buf[5]]);
        if version != VERSION {
            return Err(ProtocolError::UnsupportedVersion { version }.into());
        }

        let payload_format = buf[28];
        let _ = PayloadFormat::from_u8(payload_format)?;

        let qos = buf[29];
        let _ = Qos::from_u8(qos)?;

        let mut reserved = [0u8; 4];
        reserved.copy_from_slice(&buf[36..40]);

        Ok(Self {
            magic,
            version,
            msg_type: u16::from_le_bytes([buf[6], buf[7]]),
            msg_id: u64::from_le_bytes([
                buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
            ]),
            correlation_id: u64::from_le_bytes([
                buf[16], buf[17], buf[18], buf[19], buf[20], buf[21], buf[22], buf[23],
            ]),
            payload_len: u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]),
            payload_format,
            qos,
            credit_grant: u16::from_le_bytes([buf[30], buf[31]]),
            flags: u32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]),
            reserved,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::constants::{MsgType, VERSION};

    /// Helper: create a valid header for testing.
    fn sample_header() -> MessageHeader {
        MessageHeader {
            magic: MAGIC,
            version: VERSION,
            msg_type: MsgType::DATA.as_u16(),
            msg_id: 0x0102_0304_0506_0708,
            correlation_id: 0x1112_1314_1516_1718,
            payload_len: 1024,
            payload_format: PayloadFormat::Arrow as u8,
            qos: Qos::Normal as u8,
            credit_grant: 10,
            flags: 0,
            reserved: [0; 4],
        }
    }

    #[test]
    fn encode_decode_roundtrip() {
        let original = sample_header();
        let mut buf = [0u8; HEADER_SIZE];
        let Ok(()) = original.encode(&mut buf) else {
            panic!("encode should succeed for valid header");
        };
        let Ok(decoded) = MessageHeader::decode(&buf) else {
            panic!("decode should succeed for valid buffer");
        };
        assert_eq!(decoded, original);
    }

    #[test]
    fn encode_buffer_too_short() {
        let hdr = sample_header();
        let mut buf = [0u8; 39];
        let Err(err) = hdr.encode(&mut buf) else {
            panic!("encode should fail for 39-byte buffer");
        };
        assert_eq!(err.code().as_u16(), 0x0302);
    }

    #[test]
    fn decode_buffer_too_short() {
        let buf = [0u8; 10];
        let Err(err) = MessageHeader::decode(&buf) else {
            panic!("decode should fail for 10-byte buffer");
        };
        assert_eq!(err.code().as_u16(), 0x0302);
    }

    #[test]
    fn decode_invalid_magic() {
        let hdr = sample_header();
        let mut buf = [0u8; HEADER_SIZE];
        let Ok(()) = hdr.encode(&mut buf) else {
            panic!("encode should succeed for valid header");
        };
        // Corrupt magic.
        buf[0] = 0xFF;
        let Err(err) = MessageHeader::decode(&buf) else {
            panic!("decode should fail for corrupted magic");
        };
        assert_eq!(err.code().as_u16(), 0x0301);
    }

    #[test]
    fn decode_invalid_payload_format() {
        let mut hdr = sample_header();
        hdr.payload_format = 99;
        let mut buf = [0u8; HEADER_SIZE];
        // Encode raw bytes (bypass validation in encode).
        buf[0..4].copy_from_slice(&hdr.magic.to_le_bytes());
        buf[4..6].copy_from_slice(&hdr.version.to_le_bytes());
        buf[6..8].copy_from_slice(&hdr.msg_type.to_le_bytes());
        buf[8..16].copy_from_slice(&hdr.msg_id.to_le_bytes());
        buf[16..24].copy_from_slice(&hdr.correlation_id.to_le_bytes());
        buf[24..28].copy_from_slice(&hdr.payload_len.to_le_bytes());
        buf[28] = 99; // invalid
        buf[29] = hdr.qos;
        buf[30..32].copy_from_slice(&hdr.credit_grant.to_le_bytes());
        buf[32..36].copy_from_slice(&hdr.flags.to_le_bytes());

        let Err(err) = MessageHeader::decode(&buf) else {
            panic!("decode should fail for invalid payload_format");
        };
        assert_eq!(err.code().as_u16(), 0x0304);
    }

    #[test]
    fn decode_invalid_qos() {
        let hdr = sample_header();
        let mut buf = [0u8; HEADER_SIZE];
        let Ok(()) = hdr.encode(&mut buf) else {
            panic!("encode should succeed for valid header");
        };
        buf[29] = 5; // invalid QoS
        let Err(err) = MessageHeader::decode(&buf) else {
            panic!("decode should fail for invalid qos");
        };
        assert_eq!(err.code().as_u16(), 0x0305);
    }

    #[test]
    fn little_endian_byte_order() {
        let hdr = sample_header();
        let mut buf = [0u8; HEADER_SIZE];
        let Ok(()) = hdr.encode(&mut buf) else {
            panic!("encode should succeed for valid header");
        };

        // Magic: 0x4C414943 in LE = [0x43, 0x49, 0x41, 0x4C]
        assert_eq!(buf[0], 0x43);
        assert_eq!(buf[1], 0x49);
        assert_eq!(buf[2], 0x41);
        assert_eq!(buf[3], 0x4C);

        // msg_id: 0x0102030405060708 in LE = [0x08, 0x07, ...]
        assert_eq!(buf[8], 0x08);
        assert_eq!(buf[9], 0x07);
    }

    #[test]
    fn encode_into_larger_buffer() {
        let hdr = sample_header();
        let mut buf = [0u8; 128];
        let Ok(()) = hdr.encode(&mut buf) else {
            panic!("encode should succeed for 128-byte buffer");
        };
        // Bytes after header are untouched.
        assert!(buf[40..].iter().all(|&b| b == 0));
    }

    #[test]
    fn decode_unsupported_version() {
        let hdr = sample_header();
        let mut buf = [0u8; HEADER_SIZE];
        let Ok(()) = hdr.encode(&mut buf) else {
            panic!("encode should succeed for valid header");
        };
        // Overwrite version to 0x0002 (LE).
        buf[4] = 0x02;
        buf[5] = 0x00;
        let Err(err) = MessageHeader::decode(&buf) else {
            panic!("decode should reject unsupported version");
        };
        assert_eq!(err.code().as_u16(), 0x0303);
    }

    #[test]
    fn encode_invalid_payload_format() {
        let mut hdr = sample_header();
        hdr.payload_format = 99;
        let mut buf = [0u8; HEADER_SIZE];
        let Err(err) = hdr.encode(&mut buf) else {
            panic!("encode should reject invalid payload_format");
        };
        assert_eq!(err.code().as_u16(), 0x0304);
    }

    #[test]
    fn encode_invalid_qos() {
        let mut hdr = sample_header();
        hdr.qos = 7;
        let mut buf = [0u8; HEADER_SIZE];
        let Err(err) = hdr.encode(&mut buf) else {
            panic!("encode should reject invalid qos");
        };
        assert_eq!(err.code().as_u16(), 0x0305);
    }

    #[test]
    fn encode_invalid_magic() {
        let mut hdr = sample_header();
        hdr.magic = 0xDEAD_BEEF;
        let mut buf = [0u8; HEADER_SIZE];
        let Err(err) = hdr.encode(&mut buf) else {
            panic!("encode should reject invalid magic");
        };
        assert_eq!(err.code().as_u16(), 0x0301);
    }

    #[test]
    fn encode_invalid_version() {
        let mut hdr = sample_header();
        hdr.version = 0x9999;
        let mut buf = [0u8; HEADER_SIZE];
        let Err(err) = hdr.encode(&mut buf) else {
            panic!("encode should reject unsupported version");
        };
        assert_eq!(err.code().as_u16(), 0x0303);
    }
}
