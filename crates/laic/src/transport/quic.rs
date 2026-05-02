//! QUIC transport backend: message delivery over encrypted QUIC connections.
//!
//! Uses [Quinn](https://docs.rs/quinn) for the QUIC implementation and
//! [rustls](https://docs.rs/rustls) for TLS 1.3 / mTLS.

use std::net::SocketAddr;

use crate::error::{LaicError, TransportError};
use crate::protocol::message::Message;
use crate::transport::framing::{read_frame, write_frame};
use crate::transport::tls::{ClientTlsConfig, ServerTlsConfig};

// ---------------------------------------------------------------------------
// QuicServer
// ---------------------------------------------------------------------------

/// A QUIC server that listens for incoming mTLS connections.
///
/// Bind to an address with [`QuicServer::bind`], then call
/// [`QuicServer::accept`] to receive connections.
pub struct QuicServer {
    endpoint: quinn::Endpoint,
}

impl QuicServer {
    /// Bind a QUIC server to `addr` with mutual TLS.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::ConnectionFailed`] if the TLS config
    /// is invalid or the address cannot be bound.
    pub fn bind(addr: SocketAddr, tls: &ServerTlsConfig) -> Result<Self, LaicError> {
        let server_config = tls.build()?;
        let endpoint = quinn::Endpoint::server(server_config, addr).map_err(|e| {
            TransportError::ConnectionFailed {
                detail: format!("failed to bind QUIC server: {e}"),
            }
        })?;
        Ok(Self { endpoint })
    }

    /// Accept one incoming connection and its first bidirectional stream.
    ///
    /// Blocks until a client connects and opens a stream.
    ///
    /// CONSTRAINT: this method couples connection acceptance with stream
    /// opening — it awaits `accept_bi()` after the TLS handshake. A
    /// client that completes the handshake but never opens a bidirectional
    /// stream will block this call indefinitely. LAIC clients always call
    /// `open_bi()` inside `connect()`, so the happy path is immediate.
    /// Timeout protection is deferred to Phase 4.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::ConnectionFailed`] if the endpoint is
    /// closed or the handshake / stream negotiation fails.
    pub async fn accept(&self) -> Result<QuicConnection, LaicError> {
        let incoming =
            self.endpoint
                .accept()
                .await
                .ok_or_else(|| TransportError::ConnectionFailed {
                    detail: "server endpoint closed".into(),
                })?;
        let conn = incoming
            .await
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("incoming connection failed: {e}"),
            })?;
        let (send, recv) =
            conn.accept_bi()
                .await
                .map_err(|e| TransportError::ConnectionFailed {
                    detail: format!("failed to accept bidirectional stream: {e}"),
                })?;
        Ok(QuicConnection {
            send,
            recv,
            conn,
            _endpoint: None,
        })
    }

    /// Returns the local address this server is bound to.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::ConnectionFailed`] if the address
    /// cannot be determined (e.g. endpoint already closed).
    pub fn local_addr(&self) -> Result<SocketAddr, LaicError> {
        self.endpoint.local_addr().map_err(|e| {
            LaicError::Transport(TransportError::ConnectionFailed {
                detail: format!("failed to get local address: {e}"),
            })
        })
    }

    /// Gracefully shut down the server, refusing new connections.
    pub fn close(&self) {
        self.endpoint.close(0u32.into(), b"shutdown");
    }
}

// ---------------------------------------------------------------------------
// QuicConnection
// ---------------------------------------------------------------------------

/// A bidirectional QUIC connection for exchanging LAIC messages.
///
/// Wraps a single QUIC bidirectional stream. Messages are framed as
/// `[40-byte header][payload]` using the transport framing helpers.
pub struct QuicConnection {
    send: quinn::SendStream,
    recv: quinn::RecvStream,
    conn: quinn::Connection,
    // WHY: for client-created connections, keeps the endpoint alive so
    // the QUIC connection is not prematurely terminated. Server-accepted
    // connections set this to None (the QuicServer owns the endpoint).
    _endpoint: Option<quinn::Endpoint>,
}

impl QuicConnection {
    /// Connect to a QUIC server and open a bidirectional stream.
    ///
    /// `server_name` must match the server certificate's Subject
    /// Alternative Name (typically `"localhost"` in tests).
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::ConnectionFailed`] if the TLS config
    /// is invalid, the server is unreachable, or the handshake fails.
    pub async fn connect(
        addr: SocketAddr,
        server_name: &str,
        tls: &ClientTlsConfig,
    ) -> Result<Self, LaicError> {
        let client_config = tls.build()?;

        let bind_addr: SocketAddr = if addr.is_ipv6() {
            SocketAddr::from(([0u8; 16], 0u16))
        } else {
            SocketAddr::from(([0u8; 4], 0u16))
        };

        let mut endpoint =
            quinn::Endpoint::client(bind_addr).map_err(|e| TransportError::ConnectionFailed {
                detail: format!("failed to create client endpoint: {e}"),
            })?;
        endpoint.set_default_client_config(client_config);

        let conn = endpoint
            .connect(addr, server_name)
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("failed to initiate connection: {e}"),
            })?
            .await
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("connection failed: {e}"),
            })?;

        let (send, recv) = conn
            .open_bi()
            .await
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("failed to open bidirectional stream: {e}"),
            })?;

        Ok(Self {
            send,
            recv,
            conn,
            _endpoint: Some(endpoint),
        })
    }

    /// Send a LAIC message over this connection.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::SendFailed`] on I/O failure, or
    /// [`LaicError::Protocol`] if the message header is invalid.
    pub async fn send(&mut self, msg: &Message) -> Result<(), LaicError> {
        write_frame(&mut self.send, msg).await
    }

    /// Receive the next LAIC message from this connection.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::ReceiveFailed`] on I/O failure,
    /// [`TransportError::FramingError`] if the payload exceeds the
    /// maximum, or [`LaicError::Protocol`] for invalid header fields.
    pub async fn receive(&mut self) -> Result<Message, LaicError> {
        read_frame(&mut self.recv).await
    }

    /// Gracefully close this connection.
    ///
    /// Calls `finish()` on the send stream (signals no more data will
    /// be written — quinn flushes any buffered bytes) and then sends
    /// a QUIC `CONNECTION_CLOSE` frame. This method returns
    /// immediately and does **not** wait for the peer to acknowledge
    /// the close; stronger graceful-drain semantics (ACK-wait +
    /// timeout) are deferred to Phase 4.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::ConnectionFailed`] if the send
    /// stream cannot be finished.
    // WHY: async to match enum Transport's async interface even though
    // quinn's finish() and close() are synchronous in 0.11.
    #[allow(clippy::unused_async)]
    pub async fn close(&mut self) -> Result<(), LaicError> {
        self.send
            .finish()
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("failed to finish send stream: {e}"),
            })?;
        self.conn.close(0u32.into(), b"done");
        Ok(())
    }

    /// Returns the remote address of the peer.
    #[must_use]
    pub fn remote_addr(&self) -> SocketAddr {
        self.conn.remote_address()
    }
}
