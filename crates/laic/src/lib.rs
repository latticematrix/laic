//! LAIC — Latrix AI Interconnect.
//!
//! High-speed communication protocol for AI entities, providing local
//! zero-copy IPC via shared memory and remote transport via QUIC.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(missing_docs)]
#![warn(clippy::pedantic)]

pub mod codec;
pub mod emergency;
pub mod error;
pub mod flow;
pub mod gateway;
pub mod handshake;
pub mod pattern;
pub mod protocol;
pub mod transport;

// Re-export key types at crate root for convenience.
pub use emergency::EmergencyChannel;
pub use error::{CodecError, ErrorCode, FlowError, LaicError, ProtocolError, TransportError};
pub use flow::CreditController;
pub use gateway::Gateway;
pub use handshake::{
    client_handshake, server_handshake, ClientHandshakeConfig, ServerHandshakeConfig,
    TrustDomainSession,
};
pub use protocol::constants::{MsgType, PayloadFormat, Qos};
pub use protocol::header::MessageHeader;
pub use protocol::message::Message;
pub use transport::ipc::IpcConnection;
pub use transport::quic::{QuicConnection, QuicServer};
pub use transport::tls::{ClientTlsConfig, ServerTlsConfig};
pub use transport::Transport;
