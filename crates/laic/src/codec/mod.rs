//! Encoding and decoding for LAIC message payloads.
//!
//! - **Arrow IPC** ([`arrow`]): data-plane payloads (tensors, embeddings).
//! - **Protobuf** ([`proto`]): control-plane messages.

pub mod arrow;
pub mod proto;
