//! Communication patterns: Skill RPC, Stream, and Topic Pub/Sub.
//!
//! Provides message constructors and type constants for the three
//! communication patterns built on top of the transport layer.
//! Pattern functions create [`Message`] objects — they do not perform
//! any transport, routing, or retry logic (Mechanism, not Policy).
//!
//! # `MsgType` Numbering
//!
//! - `0x00xx` — core protocol types (defined in [`constants`])
//! - `0x001x` — Skill RPC
//! - `0x002x` — Stream
//! - `0x003x` — Topic Pub/Sub
//!
//! [`constants`]: crate::protocol::constants

use crate::error::{LaicError, ProtocolError};
use crate::protocol::constants::{MsgType, PayloadFormat, Qos};
use crate::protocol::message::Message;

// ---------------------------------------------------------------------------
// MsgType constants (open-set, per design)
// ---------------------------------------------------------------------------

// WHY: defined here (not constants.rs) per open-set MsgType design.
// constants.rs = core protocol types, pattern.rs = pattern-specific types.

/// Skill RPC request message type.
pub const SKILL_REQUEST: MsgType = MsgType::new(0x0010);

/// Skill RPC response message type.
pub const SKILL_RESPONSE: MsgType = MsgType::new(0x0011);

/// Stream data chunk message type.
pub const STREAM_DATA: MsgType = MsgType::new(0x0020);

/// Topic publish message type.
pub const TOPIC_PUBLISH: MsgType = MsgType::new(0x0030);

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

/// Create a Skill RPC request message.
///
/// The caller provides a unique `msg_id`. The `correlation_id` is left
/// at 0 — it will be set by [`skill_response`] on the response side.
#[must_use]
pub fn skill_request(msg_id: u64, format: PayloadFormat, qos: Qos, payload: Vec<u8>) -> Message {
    Message::new(SKILL_REQUEST, msg_id, format, qos, payload)
}

/// Create a Skill RPC response correlated to a request.
///
/// Sets `correlation_id = request.header().msg_id` so the requester can
/// match the response to its original request.
///
/// WHY: validates that `request` is a `SKILL_REQUEST` to enforce the
/// RPC contract — a response must always originate from a concrete
/// request; passing an arbitrary message is a programming error.
///
/// # Errors
///
/// Returns [`ProtocolError::UnexpectedMessageType`] if `request` is not
/// a `SKILL_REQUEST`.
pub fn skill_response(
    request: &Message,
    response_id: u64,
    format: PayloadFormat,
    qos: Qos,
    payload: Vec<u8>,
) -> Result<Message, LaicError> {
    if !is_skill_request(request) {
        return Err(ProtocolError::UnexpectedMessageType {
            expected: SKILL_REQUEST.as_u16(),
            actual: request.msg_type().as_u16(),
        }
        .into());
    }
    let mut msg = Message::new(SKILL_RESPONSE, response_id, format, qos, payload);
    msg.set_correlation_id(request.header().msg_id);
    Ok(msg)
}

/// Create a Stream data chunk message.
///
/// When `is_last` is `true`, sets [`FLAG_END_OF_STREAM`] to signal
/// stream termination.
///
/// [`FLAG_END_OF_STREAM`]: crate::protocol::constants::FLAG_END_OF_STREAM
#[must_use]
pub fn stream_chunk(
    msg_id: u64,
    format: PayloadFormat,
    qos: Qos,
    payload: Vec<u8>,
    is_last: bool,
) -> Message {
    let mut msg = Message::new(STREAM_DATA, msg_id, format, qos, payload);
    if is_last {
        msg.set_end_of_stream();
    }
    msg
}

/// Create a Topic publish message.
///
/// Topic routing (service name, subscriber filtering) is the caller's
/// responsibility — LAIC only provides the message type constant.
#[must_use]
pub fn topic_publish(msg_id: u64, format: PayloadFormat, qos: Qos, payload: Vec<u8>) -> Message {
    Message::new(TOPIC_PUBLISH, msg_id, format, qos, payload)
}

// ---------------------------------------------------------------------------
// Matchers
// ---------------------------------------------------------------------------

/// Check if a message is a Skill RPC request.
#[must_use]
pub fn is_skill_request(msg: &Message) -> bool {
    msg.msg_type() == SKILL_REQUEST
}

/// Check if a message is a Skill RPC response.
#[must_use]
pub fn is_skill_response(msg: &Message) -> bool {
    msg.msg_type() == SKILL_RESPONSE
}

/// Check if a message is a Stream data chunk.
#[must_use]
pub fn is_stream_data(msg: &Message) -> bool {
    msg.msg_type() == STREAM_DATA
}

/// Check if a message is a Topic publish message.
#[must_use]
pub fn is_topic_publish(msg: &Message) -> bool {
    msg.msg_type() == TOPIC_PUBLISH
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msg_type_constants_no_overlap_with_core() {
        // Core types are 0x0001..=0x0004, pattern types start at 0x0010.
        assert!(SKILL_REQUEST.as_u16() >= 0x0010);
        assert!(SKILL_RESPONSE.as_u16() >= 0x0010);
        assert!(STREAM_DATA.as_u16() >= 0x0020);
        assert!(TOPIC_PUBLISH.as_u16() >= 0x0030);
    }

    #[test]
    fn skill_request_msg_type() {
        let msg = skill_request(1, PayloadFormat::Protobuf, Qos::Normal, vec![1, 2, 3]);
        assert_eq!(msg.msg_type(), SKILL_REQUEST);
        assert_eq!(msg.header().msg_id, 1);
        assert_eq!(msg.header().correlation_id, 0);
    }

    #[test]
    fn skill_response_correlation() {
        let req = skill_request(42, PayloadFormat::Protobuf, Qos::Normal, vec![1]);
        let Ok(resp) = skill_response(&req, 100, PayloadFormat::Protobuf, Qos::Normal, vec![2])
        else {
            panic!("skill_response should succeed for a valid SKILL_REQUEST");
        };
        assert_eq!(resp.msg_type(), SKILL_RESPONSE);
        assert_eq!(resp.header().msg_id, 100);
        assert_eq!(resp.header().correlation_id, 42);
    }

    #[test]
    fn stream_chunk_without_end() {
        let msg = stream_chunk(1, PayloadFormat::Arrow, Qos::Normal, vec![0; 10], false);
        assert_eq!(msg.msg_type(), STREAM_DATA);
        assert!(!msg.is_end_of_stream());
    }

    #[test]
    fn stream_chunk_with_end() {
        let msg = stream_chunk(2, PayloadFormat::Arrow, Qos::Normal, vec![0; 5], true);
        assert_eq!(msg.msg_type(), STREAM_DATA);
        assert!(msg.is_end_of_stream());
    }

    #[test]
    fn topic_publish_msg_type() {
        let msg = topic_publish(1, PayloadFormat::Raw, Qos::High, vec![0xAB]);
        assert_eq!(msg.msg_type(), TOPIC_PUBLISH);
    }

    #[test]
    fn matchers_are_exclusive() {
        let req = skill_request(1, PayloadFormat::Protobuf, Qos::Normal, vec![]);
        let Ok(resp) = skill_response(&req, 2, PayloadFormat::Protobuf, Qos::Normal, vec![]) else {
            panic!("skill_response should succeed for a valid SKILL_REQUEST");
        };
        let chunk = stream_chunk(3, PayloadFormat::Arrow, Qos::Normal, vec![], false);
        let topic = topic_publish(4, PayloadFormat::Raw, Qos::Normal, vec![]);

        // Each matcher only matches its own type.
        assert!(is_skill_request(&req));
        assert!(!is_skill_response(&req));
        assert!(!is_stream_data(&req));
        assert!(!is_topic_publish(&req));

        assert!(!is_skill_request(&resp));
        assert!(is_skill_response(&resp));

        assert!(!is_skill_request(&chunk));
        assert!(is_stream_data(&chunk));

        assert!(!is_skill_request(&topic));
        assert!(is_topic_publish(&topic));
    }

    #[test]
    fn skill_response_carries_request_payload_format() {
        let req = skill_request(1, PayloadFormat::Protobuf, Qos::Normal, vec![1]);
        // Response can use a different format — no coupling enforced.
        let Ok(resp) = skill_response(&req, 2, PayloadFormat::Raw, Qos::High, vec![2]) else {
            panic!("skill_response should succeed for a valid SKILL_REQUEST");
        };
        assert_eq!(resp.header().payload_format, PayloadFormat::Raw as u8);
        assert_eq!(resp.header().qos, Qos::High as u8);
    }

    #[test]
    fn skill_response_rejects_non_request() {
        // Passing a TOPIC_PUBLISH as "request" must fail.
        let topic = topic_publish(1, PayloadFormat::Raw, Qos::Normal, vec![]);
        let Err(err) = skill_response(&topic, 2, PayloadFormat::Raw, Qos::Normal, vec![]) else {
            panic!("skill_response should reject non-SKILL_REQUEST");
        };
        // UnexpectedMessageType = 0x0307
        assert_eq!(err.code().as_u16(), 0x0307);
    }
}
