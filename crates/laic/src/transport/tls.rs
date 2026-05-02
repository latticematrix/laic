//! TLS configuration for QUIC transport endpoints.
//!
//! Provides [`ServerTlsConfig`] and [`ClientTlsConfig`] for setting up
//! mutual TLS (mTLS) on QUIC connections. No insecure mode exists —
//! all QUIC transport uses TLS 1.3 by design.

use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};

use crate::error::{LaicError, TransportError};

/// ALPN protocol identifier for LAIC over QUIC.
///
/// WHY: ALPN (Application-Layer Protocol Negotiation) lets the TLS
/// handshake verify that both endpoints speak the same application
/// protocol, preventing accidental cross-protocol connections.
pub(crate) const ALPN_LAIC: &[u8] = b"laic";

// ---------------------------------------------------------------------------
// ServerTlsConfig
// ---------------------------------------------------------------------------

/// TLS configuration for a QUIC server endpoint.
///
/// Requires a server certificate, private key, and a CA certificate
/// for verifying client certificates (mutual TLS).
pub struct ServerTlsConfig {
    cert_chain: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
    client_ca: CertificateDer<'static>,
}

impl ServerTlsConfig {
    /// Create a new server TLS config for mutual TLS.
    ///
    /// - `cert_chain`: server certificate chain (leaf first, then intermediates).
    /// - `key`: server private key matching the leaf certificate.
    /// - `client_ca`: CA certificate used to verify client certificates.
    #[must_use]
    pub fn new(
        cert_chain: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
        client_ca: CertificateDer<'static>,
    ) -> Self {
        Self {
            cert_chain,
            key,
            client_ca,
        }
    }

    /// Build a quinn-compatible server configuration.
    pub(crate) fn build(&self) -> Result<quinn::ServerConfig, LaicError> {
        let mut root_store = rustls::RootCertStore::empty();
        root_store
            .add(self.client_ca.clone())
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("invalid client CA certificate: {e}"),
            })?;

        let client_verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(root_store))
            .build()
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("failed to build client verifier: {e}"),
            })?;

        let mut tls_config = rustls::ServerConfig::builder()
            .with_client_cert_verifier(client_verifier)
            .with_single_cert(self.cert_chain.clone(), self.key.clone_key())
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("invalid server certificate/key: {e}"),
            })?;

        tls_config.alpn_protocols = vec![ALPN_LAIC.to_vec()];

        let quic_config =
            quinn::crypto::rustls::QuicServerConfig::try_from(tls_config).map_err(|e| {
                TransportError::ConnectionFailed {
                    detail: format!("failed to create QUIC server config: {e}"),
                }
            })?;

        Ok(quinn::ServerConfig::with_crypto(Arc::new(quic_config)))
    }
}

// ---------------------------------------------------------------------------
// ClientTlsConfig
// ---------------------------------------------------------------------------

/// TLS configuration for a QUIC client endpoint.
///
/// Requires a trusted server CA, plus a client certificate and key
/// for mutual TLS authentication.
pub struct ClientTlsConfig {
    server_ca: CertificateDer<'static>,
    cert_chain: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
}

impl ClientTlsConfig {
    /// Create a new client TLS config for mutual TLS.
    ///
    /// - `server_ca`: CA certificate to verify the server's identity.
    /// - `cert_chain`: client certificate chain for mTLS.
    /// - `key`: client private key matching the leaf certificate.
    #[must_use]
    pub fn new(
        server_ca: CertificateDer<'static>,
        cert_chain: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
    ) -> Self {
        Self {
            server_ca,
            cert_chain,
            key,
        }
    }

    /// Build a quinn-compatible client configuration.
    pub(crate) fn build(&self) -> Result<quinn::ClientConfig, LaicError> {
        let mut root_store = rustls::RootCertStore::empty();
        root_store
            .add(self.server_ca.clone())
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("invalid server CA certificate: {e}"),
            })?;

        let mut tls_config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_client_auth_cert(self.cert_chain.clone(), self.key.clone_key())
            .map_err(|e| TransportError::ConnectionFailed {
                detail: format!("invalid client certificate/key: {e}"),
            })?;

        tls_config.alpn_protocols = vec![ALPN_LAIC.to_vec()];

        let quic_config =
            quinn::crypto::rustls::QuicClientConfig::try_from(tls_config).map_err(|e| {
                TransportError::ConnectionFailed {
                    detail: format!("failed to create QUIC client config: {e}"),
                }
            })?;

        Ok(quinn::ClientConfig::new(Arc::new(quic_config)))
    }
}
