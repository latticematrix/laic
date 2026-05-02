//! QUIC stream framing: read and write LAIC message frames.
//!
//! A LAIC frame on the wire is simply:
//!
//! ```text
//! [40-byte header][payload_len bytes of payload]
//! ```
//!
//! No extra length prefix — `payload_len` in the header is the single
//! source of truth (Decision D8).

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::{LaicError, TransportError};
use crate::protocol::constants::HEADER_SIZE;
use crate::protocol::header::MessageHeader;
use crate::protocol::message::Message;

/// Maximum payload length accepted by the framing layer.
///
/// WHY: protects against allocation attacks from malicious or buggy peers.
/// 64 MiB is generous for any realistic LAIC payload (typical AI embeddings
/// are ~3 KB; large Arrow batches rarely exceed a few MB).
pub const MAX_PAYLOAD_LEN: u32 = 64 * 1024 * 1024;

/// Write a LAIC message as a frame (header + payload) to an async writer.
///
/// # Errors
///
/// - [`LaicError::Transport`] ([`TransportError::FramingError`]) if the
///   payload exceeds [`MAX_PAYLOAD_LEN`].
/// - [`LaicError::Protocol`] if the header contains invalid fields (should
///   not happen for `Message`s constructed through the public API).
/// - [`LaicError::Transport`] ([`TransportError::SendFailed`]) on I/O error.
pub async fn write_frame<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    msg: &Message,
) -> Result<(), LaicError> {
    // WHY: symmetric with read_frame's MAX_PAYLOAD_LEN check. Without this,
    // a sender could emit a frame that its own receiver would reject — a
    // fail-open contract gap (historical F1 regression, 2026-03-15).
    if msg.header().payload_len > MAX_PAYLOAD_LEN {
        return Err(TransportError::FramingError {
            detail: format!(
                "payload length {} exceeds maximum {}",
                msg.header().payload_len,
                MAX_PAYLOAD_LEN
            ),
        }
        .into());
    }

    let mut header_buf = [0u8; HEADER_SIZE];
    msg.header().encode(&mut header_buf)?;

    writer.write_all(&header_buf).await.map_err(|e| {
        LaicError::Transport(TransportError::SendFailed {
            detail: e.to_string(),
        })
    })?;

    writer.write_all(msg.payload()).await.map_err(|e| {
        LaicError::Transport(TransportError::SendFailed {
            detail: e.to_string(),
        })
    })?;

    Ok(())
}

/// Read a LAIC message frame (header + payload) from an async reader.
///
/// Reads the fixed 40-byte header, validates it, checks `payload_len`
/// against [`MAX_PAYLOAD_LEN`], then reads exactly `payload_len` bytes.
///
/// # Errors
///
/// - [`LaicError::Transport`] ([`TransportError::ReceiveFailed`]) on I/O
///   error (including unexpected EOF if the stream ends mid-frame).
/// - [`LaicError::Transport`] ([`TransportError::FramingError`]) if
///   `payload_len` exceeds [`MAX_PAYLOAD_LEN`].
/// - [`LaicError::Protocol`] if the header contains invalid protocol fields.
pub async fn read_frame<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<Message, LaicError> {
    let mut header_buf = [0u8; HEADER_SIZE];
    reader.read_exact(&mut header_buf).await.map_err(|e| {
        LaicError::Transport(TransportError::ReceiveFailed {
            detail: e.to_string(),
        })
    })?;

    let header = MessageHeader::decode(&header_buf)?;

    if header.payload_len > MAX_PAYLOAD_LEN {
        return Err(TransportError::FramingError {
            detail: format!(
                "payload length {} exceeds maximum {}",
                header.payload_len, MAX_PAYLOAD_LEN
            ),
        }
        .into());
    }

    let payload_len = header.payload_len as usize;
    let mut payload = vec![0u8; payload_len];

    if payload_len > 0 {
        reader.read_exact(&mut payload).await.map_err(|e| {
            LaicError::Transport(TransportError::ReceiveFailed {
                detail: e.to_string(),
            })
        })?;
    }

    Message::from_parts(header, payload)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::constants::{MsgType, PayloadFormat, Qos, MAGIC, VERSION};
    use tokio::io::AsyncWriteExt;

    fn sample_message(payload: Vec<u8>) -> Message {
        Message::new(
            MsgType::DATA,
            42,
            PayloadFormat::Arrow,
            Qos::Normal,
            payload,
        )
    }

    #[tokio::test]
    async fn write_read_roundtrip() {
        let msg = sample_message(vec![1, 2, 3, 4, 5]);
        let (mut client, mut server) = tokio::io::duplex(1024);

        let Ok(()) = write_frame(&mut client, &msg).await else {
            panic!("write_frame should succeed");
        };
        drop(client);

        let Ok(received) = read_frame(&mut server).await else {
            panic!("read_frame should succeed for valid frame");
        };
        assert_eq!(received.header(), msg.header());
        assert_eq!(received.payload(), msg.payload());
    }

    #[tokio::test]
    async fn roundtrip_empty_payload() {
        let msg = sample_message(vec![]);
        let (mut client, mut server) = tokio::io::duplex(1024);

        let Ok(()) = write_frame(&mut client, &msg).await else {
            panic!("write_frame should succeed");
        };
        drop(client);

        let Ok(received) = read_frame(&mut server).await else {
            panic!("read_frame should succeed for empty payload");
        };
        assert_eq!(received.payload().len(), 0);
    }

    #[tokio::test]
    async fn read_truncated_header() {
        let (mut client, mut server) = tokio::io::duplex(1024);

        let Ok(()) = client.write_all(&[0u8; 10]).await else {
            panic!("write_all should succeed on duplex");
        };
        drop(client);

        let Err(err) = read_frame(&mut server).await else {
            panic!("read_frame should fail for truncated header");
        };
        assert_eq!(err.code().as_u16(), 0x0105); // ReceiveFailed
    }

    #[tokio::test]
    async fn read_truncated_payload() {
        let msg = sample_message(vec![0; 100]);
        let mut header_buf = [0u8; HEADER_SIZE];
        let Ok(()) = msg.header().encode(&mut header_buf) else {
            panic!("encode should succeed");
        };

        let (mut client, mut server) = tokio::io::duplex(1024);
        // Write full header but only 50 of 100 payload bytes.
        let Ok(()) = client.write_all(&header_buf).await else {
            panic!("write_all should succeed");
        };
        let Ok(()) = client.write_all(&[0u8; 50]).await else {
            panic!("write_all should succeed");
        };
        drop(client);

        let Err(err) = read_frame(&mut server).await else {
            panic!("read_frame should fail for truncated payload");
        };
        assert_eq!(err.code().as_u16(), 0x0105); // ReceiveFailed
    }

    #[tokio::test]
    async fn read_payload_exceeds_max() {
        let header = MessageHeader {
            magic: MAGIC,
            version: VERSION,
            msg_type: MsgType::DATA.as_u16(),
            msg_id: 1,
            correlation_id: 0,
            payload_len: MAX_PAYLOAD_LEN + 1,
            payload_format: PayloadFormat::Arrow as u8,
            qos: Qos::Normal as u8,
            credit_grant: 0,
            flags: 0,
            reserved: [0; 4],
        };
        let mut header_buf = [0u8; HEADER_SIZE];
        let Ok(()) = header.encode(&mut header_buf) else {
            panic!("encode should succeed");
        };

        let (mut client, mut server) = tokio::io::duplex(1024);
        let Ok(()) = client.write_all(&header_buf).await else {
            panic!("write_all should succeed");
        };
        drop(client);

        let Err(err) = read_frame(&mut server).await else {
            panic!("read_frame should reject oversized payload");
        };
        assert_eq!(err.code().as_u16(), 0x0109); // FramingError
    }

    #[tokio::test]
    async fn read_invalid_magic() {
        let header_buf = [0u8; HEADER_SIZE]; // magic = 0 → invalid

        let (mut client, mut server) = tokio::io::duplex(1024);
        let Ok(()) = client.write_all(&header_buf).await else {
            panic!("write_all should succeed");
        };
        drop(client);

        let Err(err) = read_frame(&mut server).await else {
            panic!("read_frame should reject invalid magic");
        };
        // ProtocolError::InvalidMagic
        assert_eq!(err.code().as_u16(), 0x0301);
    }

    #[tokio::test]
    async fn multiple_frames_sequential() {
        let msg1 = sample_message(vec![0xAA; 10]);
        let msg2 = sample_message(vec![0xBB; 20]);
        let (mut client, mut server) = tokio::io::duplex(4096);

        let Ok(()) = write_frame(&mut client, &msg1).await else {
            panic!("write_frame msg1 should succeed");
        };
        let Ok(()) = write_frame(&mut client, &msg2).await else {
            panic!("write_frame msg2 should succeed");
        };
        drop(client);

        let Ok(r1) = read_frame(&mut server).await else {
            panic!("read_frame msg1 should succeed");
        };
        let Ok(r2) = read_frame(&mut server).await else {
            panic!("read_frame msg2 should succeed");
        };
        assert_eq!(r1.payload(), &[0xAA; 10]);
        assert_eq!(r2.payload(), &[0xBB; 20]);
    }

    /// Regression test for historical F1: `write_frame` must reject payloads
    /// exceeding `MAX_PAYLOAD_LEN`, symmetric with `read_frame`'s check.
    #[tokio::test]
    async fn write_frame_rejects_oversized_payload() {
        // Allocate MAX_PAYLOAD_LEN + 1 bytes (~64 MiB + 1).
        let oversized = vec![0u8; MAX_PAYLOAD_LEN as usize + 1];
        let msg = sample_message(oversized);

        let (mut client, _server) = tokio::io::duplex(1024);
        let Err(err) = write_frame(&mut client, &msg).await else {
            panic!("write_frame should reject oversized payload");
        };
        assert_eq!(err.code().as_u16(), 0x0109); // FramingError
    }
}
