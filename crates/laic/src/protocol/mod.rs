//! Protocol core: constants, header, and message definitions.
//!
//! WHY: named `protocol` instead of `core` to avoid shadowing the Rust
//! standard library `core` crate (used for `core::mem::size_of` etc.).

pub mod constants;
pub mod header;
pub mod message;
