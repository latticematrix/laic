//! Communication pattern integration tests.
//!
//! Validates Skill RPC request-response correlation, Stream sequences
//! with end-of-stream signaling, Topic messaging, and cross-pattern
//! matcher exclusivity.

use laic::pattern::{
    is_skill_request, is_skill_response, is_stream_data, is_topic_publish, skill_request,
    skill_response, stream_chunk, topic_publish, SKILL_REQUEST, SKILL_RESPONSE, STREAM_DATA,
    TOPIC_PUBLISH,
};
use laic::{PayloadFormat, Qos};

#[test]
fn skill_roundtrip_correlation() {
    let req = skill_request(42, PayloadFormat::Protobuf, Qos::Normal, vec![1, 2, 3]);
    assert!(is_skill_request(&req));
    assert_eq!(req.header().correlation_id, 0);

    let resp = skill_response(&req, 100, PayloadFormat::Protobuf, Qos::Normal, vec![4, 5])
        .expect("valid skill request");
    assert!(is_skill_response(&resp));
    assert_eq!(resp.header().correlation_id, 42);
    assert_eq!(resp.header().msg_id, 100);
}

#[test]
fn stream_sequence_end_of_stream() {
    let chunks: Vec<_> = (0..5)
        .map(|i| {
            let is_last = i == 4;
            stream_chunk(
                i,
                PayloadFormat::Arrow,
                Qos::Normal,
                vec![i as u8; 10],
                is_last,
            )
        })
        .collect();

    for (i, chunk) in chunks.iter().enumerate() {
        assert!(is_stream_data(chunk));
        if i < 4 {
            assert!(
                !chunk.is_end_of_stream(),
                "chunk {i} should not be end-of-stream"
            );
        } else {
            assert!(
                chunk.is_end_of_stream(),
                "chunk {i} should be end-of-stream"
            );
        }
    }
}

#[test]
fn topic_message() {
    let msg = topic_publish(1, PayloadFormat::Raw, Qos::High, vec![0xAB, 0xCD]);
    assert!(is_topic_publish(&msg));
    assert_eq!(msg.msg_type(), TOPIC_PUBLISH);
    assert_eq!(msg.payload(), &[0xAB, 0xCD]);
}

#[test]
fn mixed_pattern_types_no_cross_match() {
    let req = skill_request(1, PayloadFormat::Protobuf, Qos::Normal, vec![]);
    let resp = skill_response(&req, 2, PayloadFormat::Protobuf, Qos::Normal, vec![])
        .expect("valid skill request");
    let chunk = stream_chunk(3, PayloadFormat::Arrow, Qos::Normal, vec![], false);
    let topic = topic_publish(4, PayloadFormat::Raw, Qos::Normal, vec![]);

    let messages = [&req, &resp, &chunk, &topic];
    let expected_types = [SKILL_REQUEST, SKILL_RESPONSE, STREAM_DATA, TOPIC_PUBLISH];

    for (i, msg) in messages.iter().enumerate() {
        assert_eq!(
            msg.msg_type(),
            expected_types[i],
            "message {i} has wrong type"
        );
        // Each matcher only matches its own type.
        assert_eq!(is_skill_request(msg), i == 0);
        assert_eq!(is_skill_response(msg), i == 1);
        assert_eq!(is_stream_data(msg), i == 2);
        assert_eq!(is_topic_publish(msg), i == 3);
    }
}
