//! Complete LAIC message: header + payload.

use crate::error::{LaicError, ProtocolError};
use crate::protocol::constants::{
    MsgType, PayloadFormat, Qos, FLAG_END_OF_STREAM, FLAG_HAS_CREDIT_GRANT, HEADER_SIZE, MAGIC,
    VERSION,
};
use crate::protocol::header::MessageHeader;

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

/// A complete LAIC message consisting of a fixed header and a
/// variable-length payload.
///
/// # Invariant
///
/// `header.payload_len == payload.len()` is guaranteed by construction.
/// Fields are private to prevent independent mutation that would break
/// this invariant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    // WHY: private fields enforce the invariant that
    // header.payload_len == payload.len() at all times.
    header: MessageHeader,
    payload: Vec<u8>,
}

impl Message {
    /// Create a new message with the given parameters.
    ///
    /// Automatically fills `magic`, `version`, and `payload_len`.
    ///
    /// # Panics
    ///
    /// Panics if `payload.len()` exceeds `u32::MAX` (4 GiB). LAIC messages
    /// are not designed for payloads of this size.
    #[must_use]
    pub fn new(
        msg_type: MsgType,
        msg_id: u64,
        payload_format: PayloadFormat,
        qos: Qos,
        payload: Vec<u8>,
    ) -> Self {
        // WHY: assert instead of Result — a >4 GiB payload is a programming
        // error (SHM slots and QUIC frames are orders of magnitude smaller),
        // not a runtime condition the caller should handle gracefully.
        assert!(
            u32::try_from(payload.len()).is_ok(),
            "payload length {} exceeds u32::MAX",
            payload.len()
        );

        #[allow(clippy::cast_possible_truncation)]
        let payload_len = payload.len() as u32;

        Self {
            header: MessageHeader {
                magic: MAGIC,
                version: VERSION,
                msg_type: msg_type.as_u16(),
                msg_id,
                correlation_id: 0,
                payload_len,
                payload_format: payload_format as u8,
                qos: qos as u8,
                credit_grant: 0,
                flags: 0,
                reserved: [0; 4],
            },
            payload,
        }
    }

    /// Reconstruct a message from a decoded header and payload bytes.
    ///
    /// Validates all protocol-invariant header fields (`magic`, `version`,
    /// `payload_format`, `qos`) and the `payload_len` consistency — a
    /// public constructor must not produce a `Message` with semantically
    /// invalid state.
    ///
    /// # Errors
    ///
    /// Returns [`LaicError::Protocol`] if any header invariant is violated
    /// or if `header.payload_len` does not match `payload.len()`.
    pub fn from_parts(header: MessageHeader, payload: Vec<u8>) -> Result<Self, LaicError> {
        // WHY: validate all protocol-invariant header fields — a public
        // constructor must not produce a Message whose header would be
        // rejected by encode() or decode().
        if header.magic != MAGIC {
            return Err(ProtocolError::InvalidMagic {
                actual: header.magic,
            }
            .into());
        }
        if header.version != VERSION {
            return Err(ProtocolError::UnsupportedVersion {
                version: header.version,
            }
            .into());
        }
        PayloadFormat::from_u8(header.payload_format)?;
        Qos::from_u8(header.qos)?;

        if header.payload_len as usize != payload.len() {
            return Err(ProtocolError::PayloadLengthMismatch {
                header_len: header.payload_len,
                actual_len: payload.len(),
            }
            .into());
        }
        Ok(Self { header, payload })
    }

    /// Read-only access to the message header.
    #[must_use]
    pub fn header(&self) -> &MessageHeader {
        &self.header
    }

    /// Read-only access to the payload bytes.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Set the credit grant field and manage the `FLAG_HAS_CREDIT_GRANT` flag.
    ///
    /// When `credits > 0`, sets the flag; when `credits == 0`, clears it.
    /// This does **not** affect `payload_len` — credit grant is a header-only
    /// field.
    pub fn set_credit_grant(&mut self, credits: u16) {
        self.header.credit_grant = credits;
        if credits > 0 {
            self.header.flags |= FLAG_HAS_CREDIT_GRANT;
        } else {
            self.header.flags &= !FLAG_HAS_CREDIT_GRANT;
        }
    }

    /// Get the message type as [`MsgType`] (convenience over raw `u16`).
    #[must_use]
    pub fn msg_type(&self) -> MsgType {
        MsgType::new(self.header.msg_type)
    }

    /// Set the correlation ID for request-response matching.
    ///
    /// Used by the Pattern layer to associate Skill responses with their
    /// originating requests.
    pub fn set_correlation_id(&mut self, id: u64) {
        self.header.correlation_id = id;
    }

    /// Mark this message as the last chunk in a stream.
    pub fn set_end_of_stream(&mut self) {
        self.header.flags |= FLAG_END_OF_STREAM;
    }

    /// Clear the end-of-stream flag.
    pub fn clear_end_of_stream(&mut self) {
        self.header.flags &= !FLAG_END_OF_STREAM;
    }

    /// Check if this message is the last chunk in a stream.
    #[must_use]
    pub fn is_end_of_stream(&self) -> bool {
        self.header.flags & FLAG_END_OF_STREAM != 0
    }

    /// Decompose into header and payload for transport / codec use.
    #[must_use]
    pub fn into_parts(self) -> (MessageHeader, Vec<u8>) {
        (self.header, self.payload)
    }

    /// Total wire size: header + payload.
    #[must_use]
    pub fn wire_size(&self) -> usize {
        HEADER_SIZE + self.payload.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_message_fields() {
        let payload = vec![1, 2, 3, 4];
        let msg = Message::new(
            MsgType::DATA,
            42,
            PayloadFormat::Arrow,
            Qos::Normal,
            payload.clone(),
        );
        assert_eq!(msg.header().magic, MAGIC);
        assert_eq!(msg.header().version, VERSION);
        assert_eq!(msg.header().msg_type, MsgType::DATA.as_u16());
        assert_eq!(msg.header().msg_id, 42);
        assert_eq!(msg.header().payload_len, 4);
        assert_eq!(msg.header().payload_format, PayloadFormat::Arrow as u8);
        assert_eq!(msg.header().qos, Qos::Normal as u8);
        assert_eq!(msg.payload(), &payload[..]);
    }

    #[test]
    fn wire_size() {
        let msg = Message::new(
            MsgType::CONTROL,
            1,
            PayloadFormat::Protobuf,
            Qos::High,
            vec![0; 100],
        );
        assert_eq!(msg.wire_size(), HEADER_SIZE + 100);
    }

    #[test]
    fn empty_payload() {
        let msg = Message::new(
            MsgType::HEARTBEAT,
            0,
            PayloadFormat::Raw,
            Qos::Normal,
            vec![],
        );
        assert_eq!(msg.header().payload_len, 0);
        assert_eq!(msg.wire_size(), HEADER_SIZE);
    }

    #[test]
    fn from_parts_valid() {
        let msg = Message::new(
            MsgType::DATA,
            1,
            PayloadFormat::Arrow,
            Qos::Normal,
            vec![0xAB; 8],
        );
        let (header, payload) = msg.into_parts();
        let Ok(rebuilt) = Message::from_parts(header, payload) else {
            panic!("from_parts should succeed for matching header/payload");
        };
        assert_eq!(rebuilt.header().payload_len, 8);
        assert_eq!(rebuilt.payload().len(), 8);
    }

    #[test]
    fn from_parts_rejects_length_mismatch() {
        let msg = Message::new(
            MsgType::DATA,
            1,
            PayloadFormat::Arrow,
            Qos::Normal,
            vec![0; 10],
        );
        let (header, _) = msg.into_parts();
        // header.payload_len == 10, but provide 5 bytes
        let Err(err) = Message::from_parts(header, vec![0; 5]) else {
            panic!("from_parts should reject mismatched payload length");
        };
        assert_eq!(err.code().as_u16(), 0x0306);
    }

    #[test]
    fn into_parts_preserves_data() {
        let original_payload = vec![1, 2, 3, 4, 5];
        let msg = Message::new(
            MsgType::DATA,
            99,
            PayloadFormat::Raw,
            Qos::Emergency,
            original_payload.clone(),
        );
        let (header, payload) = msg.into_parts();
        assert_eq!(header.msg_id, 99);
        assert_eq!(payload, original_payload);
    }

    #[test]
    fn from_parts_rejects_invalid_magic() {
        let msg = Message::new(
            MsgType::DATA,
            1,
            PayloadFormat::Arrow,
            Qos::Normal,
            vec![0; 4],
        );
        let (mut header, payload) = msg.into_parts();
        header.magic = 0xDEAD_BEEF;
        let Err(err) = Message::from_parts(header, payload) else {
            panic!("from_parts should reject invalid magic");
        };
        assert_eq!(err.code().as_u16(), 0x0301);
    }

    #[test]
    fn from_parts_rejects_invalid_version() {
        let msg = Message::new(
            MsgType::DATA,
            1,
            PayloadFormat::Arrow,
            Qos::Normal,
            vec![0; 4],
        );
        let (mut header, payload) = msg.into_parts();
        header.version = 0x9999;
        let Err(err) = Message::from_parts(header, payload) else {
            panic!("from_parts should reject invalid version");
        };
        assert_eq!(err.code().as_u16(), 0x0303);
    }

    #[test]
    fn from_parts_rejects_invalid_payload_format() {
        let msg = Message::new(
            MsgType::DATA,
            1,
            PayloadFormat::Arrow,
            Qos::Normal,
            vec![0; 4],
        );
        let (mut header, payload) = msg.into_parts();
        header.payload_format = 99;
        let Err(err) = Message::from_parts(header, payload) else {
            panic!("from_parts should reject invalid payload_format");
        };
        assert_eq!(err.code().as_u16(), 0x0304);
    }

    #[test]
    fn from_parts_rejects_invalid_qos() {
        let msg = Message::new(
            MsgType::DATA,
            1,
            PayloadFormat::Arrow,
            Qos::Normal,
            vec![0; 4],
        );
        let (mut header, payload) = msg.into_parts();
        header.qos = 7;
        let Err(err) = Message::from_parts(header, payload) else {
            panic!("from_parts should reject invalid qos");
        };
        assert_eq!(err.code().as_u16(), 0x0305);
    }

    #[test]
    fn set_credit_grant_sets_flag() {
        let mut msg = Message::new(
            MsgType::DATA,
            1,
            PayloadFormat::Arrow,
            Qos::Normal,
            vec![0; 4],
        );
        assert_eq!(msg.header().credit_grant, 0);
        assert_eq!(msg.header().flags & FLAG_HAS_CREDIT_GRANT, 0);

        msg.set_credit_grant(42);
        assert_eq!(msg.header().credit_grant, 42);
        assert_ne!(msg.header().flags & FLAG_HAS_CREDIT_GRANT, 0);
    }

    #[test]
    fn set_credit_grant_clears_flag_on_zero() {
        let mut msg = Message::new(
            MsgType::DATA,
            1,
            PayloadFormat::Arrow,
            Qos::Normal,
            vec![0; 4],
        );
        msg.set_credit_grant(10);
        assert_ne!(msg.header().flags & FLAG_HAS_CREDIT_GRANT, 0);

        msg.set_credit_grant(0);
        assert_eq!(msg.header().credit_grant, 0);
        assert_eq!(msg.header().flags & FLAG_HAS_CREDIT_GRANT, 0);
    }

    #[test]
    fn msg_type_returns_correct_type() {
        let msg = Message::new(
            MsgType::DATA,
            1,
            PayloadFormat::Arrow,
            Qos::Normal,
            vec![0; 4],
        );
        assert_eq!(msg.msg_type(), MsgType::DATA);

        let msg2 = Message::new(
            MsgType::HEARTBEAT,
            2,
            PayloadFormat::Raw,
            Qos::Normal,
            vec![],
        );
        assert_eq!(msg2.msg_type(), MsgType::HEARTBEAT);
    }

    #[test]
    fn set_correlation_id() {
        let mut msg = Message::new(
            MsgType::DATA,
            1,
            PayloadFormat::Arrow,
            Qos::Normal,
            vec![0; 4],
        );
        assert_eq!(msg.header().correlation_id, 0);

        msg.set_correlation_id(12345);
        assert_eq!(msg.header().correlation_id, 12345);
    }

    #[test]
    fn set_end_of_stream_flag() {
        let mut msg = Message::new(
            MsgType::DATA,
            1,
            PayloadFormat::Arrow,
            Qos::Normal,
            vec![0; 4],
        );
        assert!(!msg.is_end_of_stream());

        msg.set_end_of_stream();
        assert!(msg.is_end_of_stream());
    }

    #[test]
    fn clear_end_of_stream_flag() {
        let mut msg = Message::new(
            MsgType::DATA,
            1,
            PayloadFormat::Arrow,
            Qos::Normal,
            vec![0; 4],
        );
        msg.set_end_of_stream();
        assert!(msg.is_end_of_stream());

        msg.clear_end_of_stream();
        assert!(!msg.is_end_of_stream());
    }

    #[test]
    fn end_of_stream_preserves_other_flags() {
        let mut msg = Message::new(
            MsgType::DATA,
            1,
            PayloadFormat::Arrow,
            Qos::Normal,
            vec![0; 4],
        );
        msg.set_credit_grant(10);
        assert_ne!(msg.header().flags & FLAG_HAS_CREDIT_GRANT, 0);

        msg.set_end_of_stream();
        // Both flags should be set.
        assert!(msg.is_end_of_stream());
        assert_ne!(msg.header().flags & FLAG_HAS_CREDIT_GRANT, 0);

        msg.clear_end_of_stream();
        // Credit grant flag must survive.
        assert!(!msg.is_end_of_stream());
        assert_ne!(msg.header().flags & FLAG_HAS_CREDIT_GRANT, 0);
    }
}
