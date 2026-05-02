//! Integration tests for the LAIC error system.
//!
//! Extracted from `error.rs` inline tests to keep the definition file
//! under the 500-line limit as new error layers are added.

use laic::{CodecError, FlowError, LaicError, ProtocolError, TransportError};

// ---------------------------------------------------------------------------
// ProtocolError tests
// ---------------------------------------------------------------------------

#[test]
fn error_codes_are_in_protocol_range() {
    let cases: &[ProtocolError] = &[
        ProtocolError::InvalidMagic { actual: 0 },
        ProtocolError::BufferTooShort {
            actual: 0,
            expected: 40,
        },
        ProtocolError::UnsupportedVersion { version: 0xFFFF },
        ProtocolError::InvalidPayloadFormat { value: 99 },
        ProtocolError::InvalidQos { value: 99 },
        ProtocolError::PayloadLengthMismatch {
            header_len: 10,
            actual_len: 20,
        },
        ProtocolError::UnexpectedMessageType {
            expected: 0x0010,
            actual: 0x0030,
        },
        ProtocolError::UnexpectedPayloadFormat {
            expected: 0x02,
            actual: 0x00,
        },
    ];
    for e in cases {
        let code = e.code().as_u16();
        assert!(
            (0x0300..0x0400).contains(&code),
            "code {code:#06X} outside 0x03xx range"
        );
    }
}

#[test]
fn error_codes_are_unique() {
    let codes: Vec<u16> = [
        ProtocolError::InvalidMagic { actual: 0 },
        ProtocolError::BufferTooShort {
            actual: 0,
            expected: 40,
        },
        ProtocolError::UnsupportedVersion { version: 0 },
        ProtocolError::InvalidPayloadFormat { value: 0 },
        ProtocolError::InvalidQos { value: 0 },
        ProtocolError::PayloadLengthMismatch {
            header_len: 0,
            actual_len: 0,
        },
        ProtocolError::UnexpectedMessageType {
            expected: 0,
            actual: 0,
        },
        ProtocolError::UnexpectedPayloadFormat {
            expected: 0,
            actual: 0,
        },
    ]
    .iter()
    .map(|e| e.code().as_u16())
    .collect();

    let mut deduped = codes.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(codes.len(), deduped.len(), "duplicate error codes found");
}

#[test]
fn protocol_errors_are_not_retryable() {
    let e = ProtocolError::InvalidMagic { actual: 0 };
    assert!(!e.is_retryable());
    assert!(!LaicError::Protocol(e).is_retryable());
}

#[test]
fn display_is_readable() {
    let e = ProtocolError::InvalidMagic {
        actual: 0xDEAD_BEEF,
    };
    let msg = format!("{e}");
    assert!(msg.contains("DEADBEEF"), "display: {msg}");

    let top = LaicError::Protocol(ProtocolError::BufferTooShort {
        actual: 10,
        expected: 40,
    });
    let msg = format!("{top}");
    assert!(msg.contains("10") && msg.contains("40"), "display: {msg}");
}

#[test]
fn error_code_display() {
    // WHY: ErrorCode inner field is private; use a known error's code
    // to test Display without relying on private constructor access.
    let code = ProtocolError::InvalidMagic { actual: 0 }.code();
    assert_eq!(format!("{code}"), "0x0301");
}

#[test]
fn laic_error_from_protocol() {
    let pe = ProtocolError::InvalidQos { value: 5 };
    let le: LaicError = pe.into();
    assert_eq!(le.code().as_u16(), 0x0305);
}

#[test]
fn handshake_error_codes_remain_stable_for_sdk_matching() {
    assert_eq!(
        ProtocolError::UnsupportedHandshakeVersion {
            expected: 1,
            actual: 2
        }
        .code()
        .as_u16(),
        0x0308
    );
    assert_eq!(
        ProtocolError::TrustDomainMismatch {
            expected: "prod-a".into(),
            actual: "prod-b".into()
        }
        .code()
        .as_u16(),
        0x0309
    );
    assert_eq!(
        ProtocolError::HandshakeNonceMismatch.code().as_u16(),
        0x030A
    );
    assert_eq!(
        ProtocolError::InvalidHandshakePayload {
            detail: "test".into()
        }
        .code()
        .as_u16(),
        0x030B
    );
}

#[test]
fn unexpected_payload_format_uses_next_free_protocol_code() {
    assert_eq!(
        ProtocolError::UnexpectedPayloadFormat {
            expected: 2,
            actual: 0
        }
        .code()
        .as_u16(),
        0x030C
    );
}

// ---------------------------------------------------------------------------
// CodecError tests
// ---------------------------------------------------------------------------

#[test]
fn codec_error_codes_are_in_range() {
    let cases: &[CodecError] = &[
        CodecError::ArrowEncode {
            detail: String::new(),
        },
        CodecError::ArrowDecode {
            detail: String::new(),
        },
        CodecError::ProtoEncode {
            detail: String::new(),
        },
        CodecError::ProtoDecode {
            detail: String::new(),
        },
    ];
    for e in cases {
        let code = e.code().as_u16();
        assert!(
            (0x0200..0x0300).contains(&code),
            "code {code:#06X} outside 0x02xx range"
        );
    }
}

#[test]
fn codec_error_codes_are_unique() {
    let codes: Vec<u16> = [
        CodecError::ArrowEncode {
            detail: String::new(),
        },
        CodecError::ArrowDecode {
            detail: String::new(),
        },
        CodecError::ProtoEncode {
            detail: String::new(),
        },
        CodecError::ProtoDecode {
            detail: String::new(),
        },
    ]
    .iter()
    .map(|e| e.code().as_u16())
    .collect();

    let mut deduped = codes.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(codes.len(), deduped.len(), "duplicate codec error codes");
}

#[test]
fn codec_errors_are_not_retryable() {
    let e = CodecError::ArrowDecode {
        detail: "test".into(),
    };
    assert!(!e.is_retryable());
    assert!(!LaicError::Codec(e).is_retryable());
}

#[test]
fn codec_error_display_is_readable() {
    let e = CodecError::ArrowEncode {
        detail: "schema mismatch".into(),
    };
    let msg = format!("{e}");
    assert!(
        msg.contains("Arrow encode") && msg.contains("schema mismatch"),
        "display: {msg}"
    );
}

#[test]
fn laic_error_from_codec() {
    let ce = CodecError::ProtoDecode {
        detail: "truncated".into(),
    };
    let le: LaicError = ce.into();
    assert_eq!(le.code().as_u16(), 0x0204);
}

// ---------------------------------------------------------------------------
// TransportError tests
// ---------------------------------------------------------------------------

#[test]
fn transport_error_codes_are_in_range() {
    let cases: &[TransportError] = &[
        TransportError::ConnectionFailed {
            detail: String::new(),
        },
        TransportError::ConnectionLost {
            detail: String::new(),
        },
        TransportError::NotConnected,
        TransportError::SendFailed {
            detail: String::new(),
        },
        TransportError::ReceiveFailed {
            detail: String::new(),
        },
        TransportError::Timeout {
            operation: String::new(),
        },
        TransportError::BackpressureFull,
        TransportError::ShuttingDown,
        TransportError::FramingError {
            detail: String::new(),
        },
    ];
    for e in cases {
        let code = e.code().as_u16();
        assert!(
            (0x0100..0x0200).contains(&code),
            "code {code:#06X} outside 0x01xx range"
        );
    }
}

#[test]
fn transport_error_codes_are_unique() {
    let codes: Vec<u16> = [
        TransportError::ConnectionFailed {
            detail: String::new(),
        },
        TransportError::ConnectionLost {
            detail: String::new(),
        },
        TransportError::NotConnected,
        TransportError::SendFailed {
            detail: String::new(),
        },
        TransportError::ReceiveFailed {
            detail: String::new(),
        },
        TransportError::Timeout {
            operation: String::new(),
        },
        TransportError::BackpressureFull,
        TransportError::ShuttingDown,
        TransportError::FramingError {
            detail: String::new(),
        },
    ]
    .iter()
    .map(|e| e.code().as_u16())
    .collect();

    let mut deduped = codes.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(
        codes.len(),
        deduped.len(),
        "duplicate transport error codes"
    );
}

#[test]
fn transport_retryable_variants() {
    // Transient errors are retryable.
    assert!(TransportError::ConnectionFailed {
        detail: String::new()
    }
    .is_retryable());
    assert!(TransportError::ConnectionLost {
        detail: String::new()
    }
    .is_retryable());
    assert!(TransportError::SendFailed {
        detail: String::new()
    }
    .is_retryable());
    assert!(TransportError::ReceiveFailed {
        detail: String::new()
    }
    .is_retryable());
    assert!(TransportError::Timeout {
        operation: String::new()
    }
    .is_retryable());
    assert!(TransportError::BackpressureFull.is_retryable());

    // Permanent errors are not retryable.
    assert!(!TransportError::NotConnected.is_retryable());
    assert!(!TransportError::ShuttingDown.is_retryable());
    assert!(!TransportError::FramingError {
        detail: String::new()
    }
    .is_retryable());
}

#[test]
fn transport_error_display_is_readable() {
    let e = TransportError::ConnectionFailed {
        detail: "refused".into(),
    };
    let msg = format!("{e}");
    assert!(
        msg.contains("connection failed") && msg.contains("refused"),
        "display: {msg}"
    );
}

#[test]
fn laic_error_from_transport() {
    let te = TransportError::Timeout {
        operation: "read".into(),
    };
    let le: LaicError = te.into();
    assert_eq!(le.code().as_u16(), 0x0106);
    assert!(le.is_retryable());
}

// ---------------------------------------------------------------------------
// FlowError tests
// ---------------------------------------------------------------------------

#[test]
fn flow_error_code_is_in_range() {
    let e = FlowError::CreditExhausted;
    let code = e.code().as_u16();
    assert!(
        (0x0400..0x0500).contains(&code),
        "code {code:#06X} outside 0x04xx range"
    );
}

#[test]
fn flow_error_code_is_unique_across_all_layers() {
    let all_codes: Vec<u16> = vec![
        // Protocol
        0x0301,
        0x0302,
        0x0303,
        0x0304,
        0x0305,
        0x0306,
        // Codec
        0x0201,
        0x0202,
        0x0203,
        0x0204,
        // Transport
        0x0101,
        0x0102,
        0x0103,
        0x0104,
        0x0105,
        0x0106,
        0x0107,
        0x0108,
        0x0109,
        // Flow
        FlowError::CreditExhausted.code().as_u16(),
    ];
    let mut deduped = all_codes.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(
        all_codes.len(),
        deduped.len(),
        "duplicate error codes found"
    );
}

#[test]
fn flow_error_is_retryable() {
    let e = FlowError::CreditExhausted;
    assert!(e.is_retryable());
    assert!(LaicError::Flow(e).is_retryable());
}

#[test]
fn flow_error_display_is_readable() {
    let e = FlowError::CreditExhausted;
    let msg = format!("{e}");
    assert!(msg.contains("credit exhausted"), "display: {msg}");

    let top = LaicError::Flow(FlowError::CreditExhausted);
    let msg = format!("{top}");
    assert!(msg.contains("flow control error"), "display: {msg}");
}

#[test]
fn laic_error_from_flow() {
    let fe = FlowError::CreditExhausted;
    let le: LaicError = fe.into();
    assert_eq!(le.code().as_u16(), 0x0401);
    assert!(le.is_retryable());
}
