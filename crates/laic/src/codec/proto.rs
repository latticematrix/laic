//! Protobuf encoding and decoding for control-plane messages.
//!
//! Thin wrappers around [`prost::Message`] that map errors into
//! [`LaicError::Codec`].

use crate::error::{CodecError, LaicError};

/// Encode a Protobuf message into bytes.
///
/// # Errors
///
/// Returns [`LaicError::Codec`] if serialization fails.
pub fn encode_proto<M: prost::Message>(msg: &M) -> Result<Vec<u8>, LaicError> {
    let mut buf = Vec::with_capacity(msg.encoded_len());
    msg.encode(&mut buf).map_err(|e| CodecError::ProtoEncode {
        detail: e.to_string(),
    })?;
    Ok(buf)
}

/// Decode bytes into a Protobuf message.
///
/// # Errors
///
/// Returns [`LaicError::Codec`] if deserialization fails.
pub fn decode_proto<M: prost::Message + Default>(bytes: &[u8]) -> Result<M, LaicError> {
    M::decode(bytes).map_err(|e| {
        CodecError::ProtoDecode {
            detail: e.to_string(),
        }
        .into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test-only message type.
    #[derive(Clone, PartialEq, prost::Message)]
    struct TestMsg {
        #[prost(string, tag = "1")]
        name: String,
        #[prost(uint32, tag = "2")]
        value: u32,
    }

    #[test]
    fn roundtrip() {
        let original = TestMsg {
            name: "hello".into(),
            value: 42,
        };
        let Ok(bytes) = encode_proto(&original) else {
            panic!("encode failed");
        };
        assert!(!bytes.is_empty());
        let Ok(decoded) = decode_proto::<TestMsg>(&bytes) else {
            panic!("decode failed");
        };
        assert_eq!(decoded, original);
    }

    #[test]
    fn decode_rejects_truncated() {
        // Valid protobuf for TestMsg, then truncate.
        let original = TestMsg {
            name: "test".into(),
            value: 99,
        };
        let Ok(bytes) = encode_proto(&original) else {
            panic!("encode failed");
        };
        // Truncate to half.
        let truncated = &bytes[..bytes.len() / 2];
        let result = decode_proto::<TestMsg>(truncated);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.code().as_u16(), 0x0204);
        }
    }

    #[test]
    fn empty_message_roundtrip() {
        let original = TestMsg {
            name: String::new(),
            value: 0,
        };
        let Ok(bytes) = encode_proto(&original) else {
            panic!("encode failed");
        };
        let Ok(decoded) = decode_proto::<TestMsg>(&bytes) else {
            panic!("decode failed");
        };
        assert_eq!(decoded, original);
    }
}
