//! Minimal trust-domain handshake over an established QUIC connection.
//!
//! WHY: mTLS proves the certificate chain and encrypts the channel, but it
//! does not give LAIC a protocol-level place to confirm each peer's trust
//! domain. This module adds that minimal mechanism without importing old-repo
//! capability negotiation, session policy, or runtime orchestration.
//! CONSTRAINT: this stays above [`QuicConnection`], not inside TLS.
//! TRAP: `client_nonce`, `server_nonce`, and `session_id` stay handshake-local
//! freshness/uniqueness markers inside an mTLS-protected channel, not auth or
//! session secrets.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::codec::proto::{decode_proto, encode_proto};
use crate::error::{LaicError, ProtocolError};
use crate::protocol::constants::{MsgType, PayloadFormat, Qos, VERSION};
use crate::protocol::message::Message;
use crate::transport::quic::QuicConnection;

const HANDSHAKE_TOKEN_LEN: usize = 16;

static HANDSHAKE_COUNTER: AtomicU64 = AtomicU64::new(1);
static HANDSHAKE_MSG_ID: AtomicU64 = AtomicU64::new(10_000);

#[derive(Clone, PartialEq, prost::Message)]
struct TrustDomainHello {
    #[prost(uint32, tag = "1")]
    protocol_version: u32,
    #[prost(string, tag = "2")]
    trust_domain: String,
    #[prost(bytes = "vec", tag = "3")]
    client_nonce: Vec<u8>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct TrustDomainHelloAck {
    #[prost(uint32, tag = "1")]
    protocol_version: u32,
    #[prost(string, tag = "2")]
    trust_domain: String,
    #[prost(bytes = "vec", tag = "3")]
    client_nonce: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    server_nonce: Vec<u8>,
    #[prost(bytes = "vec", tag = "5")]
    session_id: Vec<u8>,
    #[prost(uint32, tag = "6")]
    rejection_code: u32,
    #[prost(string, optional, tag = "7")]
    rejected_expected_remote_trust_domain: Option<String>,
}

/// Client-side configuration for the minimal trust-domain handshake.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientHandshakeConfig {
    local_trust_domain: String,
    expected_remote_trust_domain: Option<String>,
    protocol_version: u16,
}

impl ClientHandshakeConfig {
    /// Create a config using the crate's current protocol version.
    #[must_use]
    pub fn new(local_trust_domain: impl Into<String>, expected_remote: Option<&str>) -> Self {
        Self {
            local_trust_domain: local_trust_domain.into(),
            expected_remote_trust_domain: expected_remote.map(str::to_owned),
            protocol_version: VERSION,
        }
    }

    /// Override the protocol version to exercise compatibility failures.
    #[must_use]
    pub fn with_protocol_version(mut self, protocol_version: u16) -> Self {
        self.protocol_version = protocol_version;
        self
    }
}

/// Server-side configuration for the minimal trust-domain handshake.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerHandshakeConfig {
    local_trust_domain: String,
    expected_remote_trust_domain: Option<String>,
    protocol_version: u16,
}

impl ServerHandshakeConfig {
    /// Create a config using the crate's current protocol version.
    #[must_use]
    pub fn new(local_trust_domain: impl Into<String>, expected_remote: Option<&str>) -> Self {
        Self {
            local_trust_domain: local_trust_domain.into(),
            expected_remote_trust_domain: expected_remote.map(str::to_owned),
            protocol_version: VERSION,
        }
    }

    /// Override the protocol version to exercise compatibility failures.
    #[must_use]
    pub fn with_protocol_version(mut self, protocol_version: u16) -> Self {
        self.protocol_version = protocol_version;
        self
    }
}

/// Successful outcome of the minimal trust-domain handshake.
/// WHY / CONSTRAINT: this only carries trust-domain metadata plus
/// handshake-local freshness markers, not session-manager or auth state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustDomainSession {
    protocol_version: u16,
    local_trust_domain: String,
    remote_trust_domain: String,
    client_nonce: [u8; HANDSHAKE_TOKEN_LEN],
    server_nonce: [u8; HANDSHAKE_TOKEN_LEN],
    session_id: [u8; HANDSHAKE_TOKEN_LEN],
}

impl TrustDomainSession {
    /// Negotiated LAIC protocol version.
    #[must_use]
    pub const fn protocol_version(&self) -> u16 {
        self.protocol_version
    }

    /// Local trust-domain name used in this handshake.
    #[must_use]
    pub fn local_trust_domain(&self) -> &str {
        &self.local_trust_domain
    }

    /// Remote trust-domain name observed from the peer.
    #[must_use]
    pub fn remote_trust_domain(&self) -> &str {
        &self.remote_trust_domain
    }

    /// Echoed handshake-local freshness marker, not an authentication secret.
    #[must_use]
    pub const fn client_nonce(&self) -> &[u8; HANDSHAKE_TOKEN_LEN] {
        &self.client_nonce
    }

    /// Server-generated handshake-local freshness marker, not a bearer secret.
    #[must_use]
    pub const fn server_nonce(&self) -> &[u8; HANDSHAKE_TOKEN_LEN] {
        &self.server_nonce
    }

    /// Minimal handshake identifier, not a policy-layer session secret/token.
    #[must_use]
    pub const fn session_id(&self) -> &[u8; HANDSHAKE_TOKEN_LEN] {
        &self.session_id
    }
}

/// Run the client side of the minimal trust-domain handshake.
///
/// Sends a `Hello`, receives a `HelloAck`, and validates protocol version,
/// remote trust-domain expectation, and echoed nonce.
///
/// # Errors
///
/// Returns [`LaicError::Protocol`] if the peer responds with an incompatible
/// version, unexpected trust-domain, or malformed handshake payload.
pub async fn client_handshake(
    conn: &mut QuicConnection,
    config: ClientHandshakeConfig,
) -> Result<TrustDomainSession, LaicError> {
    validate_trust_domain("client hello trust_domain", &config.local_trust_domain)?;
    let client_nonce = next_handshake_token();
    let hello = TrustDomainHello {
        protocol_version: u32::from(config.protocol_version),
        trust_domain: config.local_trust_domain.clone(),
        client_nonce: client_nonce.to_vec(),
    };
    send_control_message(conn, encode_proto(&hello)?).await?;

    let ack_message = conn.receive().await?;
    let ack: TrustDomainHelloAck = decode_control_message(&ack_message)?;
    validate_protocol_version(config.protocol_version, ack.protocol_version)?;
    validate_trust_domain("hello_ack trust_domain", &ack.trust_domain)?;
    validate_expected_domain(
        config.expected_remote_trust_domain.as_deref(),
        &ack.trust_domain,
    )?;
    validate_ack_rejection(&config, &ack)?;

    let echoed_client_nonce = decode_token("client_nonce", &ack.client_nonce)?;
    if echoed_client_nonce != client_nonce {
        return Err(ProtocolError::HandshakeNonceMismatch.into());
    }

    let server_nonce = decode_token("server_nonce", &ack.server_nonce)?;
    let session_id = decode_token("session_id", &ack.session_id)?;

    Ok(TrustDomainSession {
        protocol_version: config.protocol_version,
        local_trust_domain: config.local_trust_domain,
        remote_trust_domain: ack.trust_domain,
        client_nonce,
        server_nonce,
        session_id,
    })
}

/// Run the server side of the minimal trust-domain handshake.
///
/// Receives a `Hello`, sends a `HelloAck`, then finalizes the server-side
/// outcome from the same validation result.
///
/// TRADEOFF: the server still sends `HelloAck` before returning compatibility
/// failures, but the ack now carries an explicit rejection code when the
/// server refuses the client's version or claimed trust-domain. That preserves
/// a stable protocol error on the client side instead of degrading into a
/// transport-level connection drop.
///
/// # Errors
///
/// Returns [`LaicError::Protocol`] if the client's version or trust-domain
/// does not match the server's expected constraints.
pub async fn server_handshake(
    conn: &mut QuicConnection,
    config: ServerHandshakeConfig,
) -> Result<TrustDomainSession, LaicError> {
    validate_trust_domain("server hello_ack trust_domain", &config.local_trust_domain)?;
    let hello_message = conn.receive().await?;
    let hello: TrustDomainHello = decode_control_message(&hello_message)?;
    let client_nonce = decode_token("client_nonce", &hello.client_nonce)?;
    match validate_peer_hello(&config, &hello) {
        Ok(()) => {
            let server_nonce = next_handshake_token();
            let session_id = next_handshake_token();
            let ack = TrustDomainHelloAck {
                protocol_version: u32::from(config.protocol_version),
                trust_domain: config.local_trust_domain.clone(),
                client_nonce: client_nonce.to_vec(),
                server_nonce: server_nonce.to_vec(),
                session_id: session_id.to_vec(),
                rejection_code: 0,
                rejected_expected_remote_trust_domain: None,
            };
            send_control_message(conn, encode_proto(&ack)?).await?;

            Ok(TrustDomainSession {
                protocol_version: config.protocol_version,
                local_trust_domain: config.local_trust_domain,
                remote_trust_domain: hello.trust_domain,
                client_nonce,
                server_nonce,
                session_id,
            })
        }
        Err(err) => {
            // WHY: a rejection ack must stay a pure refusal signal. Fabricating
            // session material here would imply a partial session and blur the
            // boundary between protocol compatibility and later lifecycle work.
            let ack = TrustDomainHelloAck {
                protocol_version: u32::from(config.protocol_version),
                trust_domain: config.local_trust_domain.clone(),
                client_nonce: client_nonce.to_vec(),
                server_nonce: Vec::new(),
                session_id: Vec::new(),
                rejection_code: u32::from(err.code().as_u16()),
                rejected_expected_remote_trust_domain: handshake_rejected_expected_domain(&err),
            };
            send_control_message(conn, encode_proto(&ack)?).await?;
            Err(err)
        }
    }
}

async fn send_control_message(
    conn: &mut QuicConnection,
    payload: Vec<u8>,
) -> Result<(), LaicError> {
    let message = Message::new(
        MsgType::CONTROL,
        HANDSHAKE_MSG_ID.fetch_add(1, Ordering::Relaxed),
        PayloadFormat::Protobuf,
        Qos::High,
        payload,
    );
    conn.send(&message).await
}

fn decode_control_message<M>(message: &Message) -> Result<M, LaicError>
where
    M: prost::Message + Default,
{
    if message.msg_type() != MsgType::CONTROL {
        return Err(ProtocolError::UnexpectedMessageType {
            expected: MsgType::CONTROL.as_u16(),
            actual: message.msg_type().as_u16(),
        }
        .into());
    }

    let actual_format = PayloadFormat::from_u8(message.header().payload_format)?;
    if actual_format != PayloadFormat::Protobuf {
        return Err(ProtocolError::UnexpectedPayloadFormat {
            expected: PayloadFormat::Protobuf as u8,
            actual: actual_format as u8,
        }
        .into());
    }

    // WHY: control-plane protobuf is the wire-level contract here. `Qos::High`
    // stays sender-preference only because transport priority does not change
    // handshake semantics when the same protobuf arrives over another lane.
    decode_proto(message.payload()).map_err(|err| {
        // WHY: once the frame passes handshake envelope checks, decode failure
        // belongs to the handshake payload contract, so peers still see 0x030B.
        ProtocolError::InvalidHandshakePayload {
            detail: format!("handshake protobuf decode failed: {err}"),
        }
        .into()
    })
}

fn validate_peer_hello(
    config: &ServerHandshakeConfig,
    hello: &TrustDomainHello,
) -> Result<(), LaicError> {
    validate_trust_domain("hello trust_domain", &hello.trust_domain)?;
    validate_protocol_version(config.protocol_version, hello.protocol_version)?;
    validate_expected_domain(
        config.expected_remote_trust_domain.as_deref(),
        &hello.trust_domain,
    )
}

fn handshake_rejected_expected_domain(err: &LaicError) -> Option<String> {
    match err {
        LaicError::Protocol(ProtocolError::TrustDomainMismatch { expected, .. }) => {
            Some(expected.clone())
        }
        _ => None,
    }
}

fn validate_ack_rejection(
    config: &ClientHandshakeConfig,
    ack: &TrustDomainHelloAck,
) -> Result<(), LaicError> {
    const UNSUPPORTED_HANDSHAKE_VERSION_CODE: u32 = 0x0308;
    const TRUST_DOMAIN_MISMATCH_CODE: u32 = 0x0309;
    const INVALID_HANDSHAKE_PAYLOAD_CODE: u32 = 0x030B;

    validate_ack_shape(ack)?;

    match ack.rejection_code {
        0 => Ok(()),
        UNSUPPORTED_HANDSHAKE_VERSION_CODE => {
            let actual_version = u16::try_from(ack.protocol_version).map_err(|_| {
                ProtocolError::InvalidHandshakePayload {
                    detail: format!(
                        "hello_ack rejection echoed protocol_version {} outside u16",
                        ack.protocol_version
                    ),
                }
            })?;
            if actual_version == config.protocol_version {
                return Err(ProtocolError::InvalidHandshakePayload {
                    detail: "hello_ack rejected protocol version but echoed a matching version"
                        .into(),
                }
                .into());
            }
            Err(ProtocolError::UnsupportedHandshakeVersion {
                expected: config.protocol_version,
                actual: actual_version,
            }
            .into())
        }
        TRUST_DOMAIN_MISMATCH_CODE => {
            let expected = ack
                .rejected_expected_remote_trust_domain
                .as_ref()
                .ok_or_else(|| ProtocolError::InvalidHandshakePayload {
                    detail: "hello_ack trust-domain rejection omitted expected remote domain"
                        .into(),
                })?;
            Err(ProtocolError::TrustDomainMismatch {
                expected: expected.clone(),
                actual: config.local_trust_domain.clone(),
            }
            .into())
        }
        INVALID_HANDSHAKE_PAYLOAD_CODE => Err(ProtocolError::InvalidHandshakePayload {
            detail: "server rejected client hello as malformed".into(),
        }
        .into()),
        code => Err(ProtocolError::InvalidHandshakePayload {
            detail: format!("unknown hello_ack rejection code 0x{code:04X}"),
        }
        .into()),
    }
}

fn validate_ack_shape(ack: &TrustDomainHelloAck) -> Result<(), LaicError> {
    if ack.rejection_code == 0 {
        if ack.rejected_expected_remote_trust_domain.is_some() {
            return Err(ProtocolError::InvalidHandshakePayload {
                detail: "hello_ack success must not include rejection explanation".into(),
            }
            .into());
        }
        decode_token("server_nonce", &ack.server_nonce)?;
        decode_token("session_id", &ack.session_id)?;
        return Ok(());
    }

    if !ack.server_nonce.is_empty() {
        return Err(ProtocolError::InvalidHandshakePayload {
            detail: "hello_ack rejection must not include server_nonce".into(),
        }
        .into());
    }
    if !ack.session_id.is_empty() {
        return Err(ProtocolError::InvalidHandshakePayload {
            detail: "hello_ack rejection must not include session_id".into(),
        }
        .into());
    }
    Ok(())
}

fn validate_protocol_version(expected: u16, actual: u32) -> Result<(), LaicError> {
    let Ok(actual_version) = u16::try_from(actual) else {
        return Err(ProtocolError::InvalidHandshakePayload {
            detail: format!("protocol_version {actual} exceeds u16"),
        }
        .into());
    };
    if actual_version != expected {
        return Err(ProtocolError::UnsupportedHandshakeVersion {
            expected,
            actual: actual_version,
        }
        .into());
    }
    Ok(())
}

fn validate_expected_domain(expected: Option<&str>, actual: &str) -> Result<(), LaicError> {
    if let Some(expected_domain) = expected {
        if expected_domain != actual {
            return Err(ProtocolError::TrustDomainMismatch {
                expected: expected_domain.to_owned(),
                actual: actual.to_owned(),
            }
            .into());
        }
    }
    Ok(())
}

fn validate_trust_domain(field: &'static str, value: &str) -> Result<(), LaicError> {
    if value.is_empty() {
        return Err(ProtocolError::InvalidHandshakePayload {
            detail: format!("{field} must not be empty"),
        }
        .into());
    }
    Ok(())
}

fn decode_token(field: &'static str, bytes: &[u8]) -> Result<[u8; HANDSHAKE_TOKEN_LEN], LaicError> {
    if bytes.len() != HANDSHAKE_TOKEN_LEN {
        return Err(ProtocolError::InvalidHandshakePayload {
            detail: format!(
                "{field} must be {HANDSHAKE_TOKEN_LEN} bytes, got {}",
                bytes.len()
            ),
        }
        .into());
    }
    let mut token = [0u8; HANDSHAKE_TOKEN_LEN];
    token.copy_from_slice(bytes);
    Ok(token)
}

fn next_handshake_token() -> [u8; HANDSHAKE_TOKEN_LEN] {
    // WHY: the handshake only needs freshness/uniqueness inside an
    // mTLS-protected channel, so time + monotonic counter is sufficient.
    let counter = u128::from(HANDSHAKE_COUNTER.fetch_add(1, Ordering::Relaxed));
    let nanos = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(_) => 0,
    };
    (nanos ^ (counter << 64)).to_be_bytes()
}

#[cfg(test)]
mod tests;
