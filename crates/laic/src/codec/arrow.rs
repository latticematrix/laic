//! Arrow IPC encoding and decoding for data-plane payloads.
//!
//! Each LAIC Arrow message carries exactly **one** [`RecordBatch`].

use arrow_array::RecordBatch;
use arrow_ipc::reader::StreamReader;
use arrow_ipc::writer::StreamWriter;
use std::io::Cursor;

use crate::error::{CodecError, LaicError};

/// Encode a [`RecordBatch`] into Arrow IPC stream bytes.
///
/// # Errors
///
/// Returns [`LaicError::Codec`] if serialization fails.
pub fn encode_record_batch(batch: &RecordBatch) -> Result<Vec<u8>, LaicError> {
    let schema = batch.schema();
    let mut buf = Vec::new();
    {
        let mut writer =
            StreamWriter::try_new(&mut buf, &schema).map_err(|e| CodecError::ArrowEncode {
                detail: e.to_string(),
            })?;
        writer.write(batch).map_err(|e| CodecError::ArrowEncode {
            detail: e.to_string(),
        })?;
        writer.finish().map_err(|e| CodecError::ArrowEncode {
            detail: e.to_string(),
        })?;
    }
    Ok(buf)
}

/// Decode Arrow IPC stream bytes into a single [`RecordBatch`].
///
/// The stream must contain exactly one batch.
///
/// # Errors
///
/// Returns [`LaicError::Codec`] if deserialization fails or the stream
/// does not contain exactly one batch.
pub fn decode_record_batch(bytes: &[u8]) -> Result<RecordBatch, LaicError> {
    let mut reader =
        StreamReader::try_new(Cursor::new(bytes), None).map_err(|e| CodecError::ArrowDecode {
            detail: e.to_string(),
        })?;

    // WHY: fail-fast — read exactly one batch, reject immediately on 0 or >1.
    // Must distinguish Some(Ok) from Some(Err) to avoid masking decode errors
    // as cardinality errors (M2-C1).
    let first = reader
        .next()
        .ok_or_else(|| CodecError::ArrowDecode {
            detail: "expected 1 record batch, got 0".into(),
        })?
        .map_err(|e| CodecError::ArrowDecode {
            detail: e.to_string(),
        })?;

    match reader.next() {
        None => Ok(first),
        Some(Ok(_)) => Err(CodecError::ArrowDecode {
            detail: "expected 1 record batch, got >1".into(),
        }
        .into()),
        Some(Err(e)) => Err(CodecError::ArrowDecode {
            detail: e.to_string(),
        }
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{Float32Array, Int32Array};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

    fn sample_batch() -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int32, false),
            Field::new("value", DataType::Float32, false),
        ]));
        let ids = Int32Array::from(vec![1, 2, 3]);
        let vals = Float32Array::from(vec![1.0f32, 2.0, 3.0]);
        let Ok(batch) = RecordBatch::try_new(schema, vec![Arc::new(ids), Arc::new(vals)]) else {
            panic!("test batch creation failed");
        };
        batch
    }

    #[test]
    fn roundtrip() {
        let original = sample_batch();
        let Ok(bytes) = encode_record_batch(&original) else {
            panic!("encode failed");
        };
        let Ok(decoded) = decode_record_batch(&bytes) else {
            panic!("decode failed");
        };
        assert_eq!(decoded, original);
    }

    #[test]
    fn decode_rejects_garbage() {
        // WHY: 0xFF bytes would be parsed as 4GB IPC metadata length,
        // causing allocation panic.  Use zeros instead — not a valid
        // IPC continuation marker.
        let result = decode_record_batch(&[0x00; 16]);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.code().as_u16(), 0x0202);
        }
    }

    #[test]
    fn decode_rejects_empty() {
        let result = decode_record_batch(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn roundtrip_empty_batch() {
        let schema = Arc::new(Schema::new(vec![Field::new("x", DataType::Float32, false)]));
        let col = Float32Array::from(Vec::<f32>::new());
        let Ok(batch) = RecordBatch::try_new(schema, vec![Arc::new(col)]) else {
            panic!("empty batch creation failed");
        };
        let Ok(bytes) = encode_record_batch(&batch) else {
            panic!("encode empty batch failed");
        };
        let Ok(decoded) = decode_record_batch(&bytes) else {
            panic!("decode empty batch failed");
        };
        assert_eq!(decoded.num_rows(), 0);
    }

    #[test]
    fn decode_rejects_multi_batch() {
        let schema = Arc::new(Schema::new(vec![Field::new("x", DataType::Float32, false)]));
        let col = Float32Array::from(vec![1.0f32]);
        let Ok(batch) = RecordBatch::try_new(schema.clone(), vec![Arc::new(col)]) else {
            panic!("batch creation failed");
        };
        // Write two valid batches into one stream.
        let mut buf = Vec::new();
        {
            let Ok(mut writer) = StreamWriter::try_new(&mut buf, &schema) else {
                panic!("writer creation failed");
            };
            let Ok(()) = writer.write(&batch) else {
                panic!("first write failed");
            };
            let Ok(()) = writer.write(&batch) else {
                panic!("second write failed");
            };
            let Ok(()) = writer.finish() else {
                panic!("finish failed");
            };
        }
        let result = decode_record_batch(&buf);
        assert!(result.is_err());
        if let Err(e) = &result {
            let msg = format!("{e}");
            assert!(msg.contains(">1"), "should report >1 batch: {msg}");
        }
    }

    #[test]
    fn decode_corrupt_trailing_reports_decode_error_not_cardinality() {
        // Regression test for M2-C1: corrupt trailing data must produce a
        // decode error, not be masked as "got >1".
        let original = sample_batch();
        let Ok(mut bytes) = encode_record_batch(&original) else {
            panic!("encode failed");
        };
        // Arrow IPC stream ends with EOS marker: 0xFFFFFFFF + 0x00000000 (8 bytes).
        // Replace EOS with a continuation marker + absurd metadata length
        // so the reader attempts to read a second message and fails.
        let eos_start = bytes.len() - 8;
        bytes.truncate(eos_start);
        // Continuation marker (valid).
        bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        // Metadata length pointing past end of buffer → forces read error.
        bytes.extend_from_slice(&0x0000_1000u32.to_le_bytes());

        let result = decode_record_batch(&bytes);
        assert!(result.is_err());
        if let Err(e) = &result {
            let msg = format!("{e}");
            // Must NOT say "got >1" — that would be the old masking bug.
            assert!(
                !msg.contains("got >1"),
                "corrupt trailing data should not report cardinality error: {msg}"
            );
        }
    }
}
