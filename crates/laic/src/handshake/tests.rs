use super::*;

fn sample_hello() -> TrustDomainHello {
    TrustDomainHello {
        protocol_version: u32::from(VERSION),
        trust_domain: "prod-a".to_string(),
        client_nonce: [7u8; HANDSHAKE_TOKEN_LEN].to_vec(),
    }
}

fn sample_client_config() -> ClientHandshakeConfig {
    ClientHandshakeConfig::new("prod-a", Some("prod-b"))
}

fn sample_server_config() -> ServerHandshakeConfig {
    ServerHandshakeConfig::new("prod-b", None)
}

fn sample_success_ack() -> TrustDomainHelloAck {
    TrustDomainHelloAck {
        protocol_version: u32::from(VERSION),
        trust_domain: "prod-b".to_string(),
        client_nonce: [7u8; HANDSHAKE_TOKEN_LEN].to_vec(),
        server_nonce: [8u8; HANDSHAKE_TOKEN_LEN].to_vec(),
        session_id: [9u8; HANDSHAKE_TOKEN_LEN].to_vec(),
        rejection_code: 0,
        rejected_expected_remote_trust_domain: None,
    }
}

fn sample_rejection_ack() -> TrustDomainHelloAck {
    TrustDomainHelloAck {
        rejection_code: 0x0309,
        rejected_expected_remote_trust_domain: Some("prod-a".to_string()),
        ..sample_success_ack()
    }
}

#[test]
fn hello_protobuf_roundtrip() {
    let hello = sample_hello();
    let Ok(encoded) = encode_proto(&hello) else {
        panic!("encode hello");
    };
    let Ok(decoded) = decode_proto::<TrustDomainHello>(&encoded) else {
        panic!("decode hello");
    };
    assert_eq!(decoded, hello);
}

#[test]
fn decode_control_message_rejects_non_control_type() {
    let Ok(payload) = encode_proto(&sample_hello()) else {
        panic!("encode hello");
    };
    let message = Message::new(
        MsgType::DATA,
        1,
        PayloadFormat::Protobuf,
        Qos::High,
        payload,
    );

    let Err(err) = decode_control_message::<TrustDomainHello>(&message) else {
        panic!("non-control message must fail");
    };
    assert_eq!(err.code().as_u16(), 0x0307);
}

#[test]
fn decode_control_message_rejects_non_protobuf_control_payload() {
    let Ok(payload) = encode_proto(&sample_hello()) else {
        panic!("encode hello");
    };
    let message = Message::new(MsgType::CONTROL, 1, PayloadFormat::Raw, Qos::High, payload);

    let Err(err) = decode_control_message::<TrustDomainHello>(&message) else {
        panic!("handshake control messages must reject non-Protobuf payload formats");
    };
    assert_eq!(err.code().as_u16(), 0x030C);
}

#[test]
fn decode_control_message_rejects_malformed_protobuf_handshake_payload() {
    let message = Message::new(
        MsgType::CONTROL,
        1,
        PayloadFormat::Protobuf,
        Qos::High,
        vec![0x08],
    );

    let Err(err) = decode_control_message::<TrustDomainHello>(&message) else {
        panic!("malformed protobuf payload must fail as handshake payload");
    };
    assert_eq!(err.code().as_u16(), 0x030B);
}

#[test]
fn validate_protocol_version_rejects_mismatch() {
    let Err(err) = validate_protocol_version(VERSION, u32::from(VERSION) + 1) else {
        panic!("version mismatch must fail");
    };
    assert_eq!(err.code().as_u16(), 0x0308);
}

#[test]
fn validate_expected_domain_rejects_mismatch() {
    let Err(err) = validate_expected_domain(Some("prod-a"), "prod-b") else {
        panic!("domain mismatch must fail");
    };
    assert_eq!(err.code().as_u16(), 0x0309);
}

#[test]
fn validate_peer_hello_rejects_empty_trust_domain() {
    let mut hello = sample_hello();
    hello.trust_domain.clear();

    let Err(err) = validate_peer_hello(&sample_server_config(), &hello) else {
        panic!("empty trust domain must fail");
    };
    assert_eq!(err.code().as_u16(), 0x030B);
}

#[test]
fn decode_token_rejects_wrong_length() {
    let Err(err) = decode_token("session_id", &[1, 2, 3]) else {
        panic!("short token must fail");
    };
    assert_eq!(err.code().as_u16(), 0x030B);
}

#[test]
fn validate_ack_rejection_requires_expected_domain_for_trust_mismatch() {
    let mut ack = sample_rejection_ack();
    ack.server_nonce.clear();
    ack.session_id.clear();
    ack.rejected_expected_remote_trust_domain = None;

    let Err(err) = validate_ack_rejection(&sample_client_config(), &ack) else {
        panic!("trust-domain rejection must name the expected remote domain");
    };
    assert_eq!(err.code().as_u16(), 0x030B);
}

#[test]
fn validate_ack_rejection_rejects_session_material_on_rejection() {
    let ack = sample_rejection_ack();

    let Err(err) = validate_ack_rejection(&sample_client_config(), &ack) else {
        panic!("rejection ack carrying session material must fail");
    };
    assert_eq!(err.code().as_u16(), 0x030B);
}

#[test]
fn validate_ack_rejection_success_requires_server_nonce() {
    let mut ack = sample_success_ack();
    ack.server_nonce.clear();

    let Err(err) = validate_ack_rejection(&sample_client_config(), &ack) else {
        panic!("success ack missing server nonce must fail");
    };
    assert_eq!(err.code().as_u16(), 0x030B);
}

#[test]
fn validate_ack_rejection_success_requires_session_id() {
    let mut ack = sample_success_ack();
    ack.session_id.clear();

    let Err(err) = validate_ack_rejection(&sample_client_config(), &ack) else {
        panic!("success ack missing session id must fail");
    };
    assert_eq!(err.code().as_u16(), 0x030B);
}

#[test]
fn validate_ack_rejection_success_rejects_rejection_explanation() {
    let mut ack = sample_success_ack();
    ack.rejected_expected_remote_trust_domain = Some("prod-c".to_string());

    let Err(err) = validate_ack_rejection(&sample_client_config(), &ack) else {
        panic!("success ack carrying rejection explanation must fail");
    };
    assert_eq!(err.code().as_u16(), 0x030B);
}
