//! Negative tests for laicc-generated `from_arrow_ipc()` validation.
//!
//! F1: metadata validation (skill_id, version, direction)
//! F2: cardinality enforcement (exactly 1 batch, exactly 1 row)

#![allow(clippy::unwrap_used)]

use std::collections::HashMap;
use std::sync::Arc;

use arrow_array::builder::StringBuilder;
use arrow_array::RecordBatch;
use arrow_schema::{Field, Schema};

#[allow(unused)]
mod echo_laic {
    include!(concat!(env!("OUT_DIR"), "/echo_laic.rs"));
}

// ── F1: metadata validation negative tests ──

/// Build an IPC stream with 1 row, 1 batch, custom schema metadata.
fn build_echo_ipc_with_metadata(meta: HashMap<String, String>) -> Vec<u8> {
    let schema = Schema::new_with_metadata(
        vec![Field::new("text", arrow_schema::DataType::Utf8, false)],
        meta,
    );
    let mut builder = StringBuilder::new();
    builder.append_value("hello");
    let batch = RecordBatch::try_new(Arc::new(schema), vec![Arc::new(builder.finish())]).unwrap();
    let mut buf = Vec::new();
    {
        let mut writer =
            arrow_ipc::writer::StreamWriter::try_new(&mut buf, &batch.schema()).unwrap();
        writer.write(&batch).unwrap();
        writer.finish().unwrap();
    }
    buf
}

#[test]
fn f1_reject_missing_metadata() {
    // No laic.* keys at all
    let bytes = build_echo_ipc_with_metadata(HashMap::new());
    let err = echo_laic::EchoInput::from_arrow_ipc(&bytes)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("missing required metadata key"),
        "expected missing metadata error, got: {err}"
    );
}

#[test]
fn f1_reject_wrong_skill_id() {
    let meta = HashMap::from([
        ("laic.skill_id".into(), "wrong_skill".into()),
        ("laic.version".into(), "1.0.0".into()),
        ("laic.direction".into(), "input".into()),
    ]);
    let bytes = build_echo_ipc_with_metadata(meta);
    let err = echo_laic::EchoInput::from_arrow_ipc(&bytes)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("metadata 'laic.skill_id' mismatch"),
        "expected skill_id mismatch, got: {err}"
    );
}

#[test]
fn f1_reject_wrong_version() {
    let meta = HashMap::from([
        ("laic.skill_id".into(), "echo".into()),
        ("laic.version".into(), "9.9.9".into()),
        ("laic.direction".into(), "input".into()),
    ]);
    let bytes = build_echo_ipc_with_metadata(meta);
    let err = echo_laic::EchoInput::from_arrow_ipc(&bytes)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("metadata 'laic.version' mismatch"),
        "expected version mismatch, got: {err}"
    );
}

#[test]
fn f1_reject_wrong_direction() {
    let meta = HashMap::from([
        ("laic.skill_id".into(), "echo".into()),
        ("laic.version".into(), "1.0.0".into()),
        ("laic.direction".into(), "output".into()),
    ]);
    let bytes = build_echo_ipc_with_metadata(meta);
    let err = echo_laic::EchoInput::from_arrow_ipc(&bytes)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("metadata 'laic.direction' mismatch"),
        "expected direction mismatch, got: {err}"
    );
}

// ── F2: cardinality enforcement negative tests ──

/// Build an IPC stream with correct metadata but N rows in a single batch.
fn build_echo_ipc_n_rows(n: usize) -> Vec<u8> {
    let schema = Schema::new_with_metadata(
        vec![Field::new("text", arrow_schema::DataType::Utf8, false)],
        HashMap::from([
            ("laic.skill_id".into(), "echo".into()),
            ("laic.version".into(), "1.0.0".into()),
            ("laic.direction".into(), "input".into()),
        ]),
    );
    let mut builder = StringBuilder::new();
    for i in 0..n {
        builder.append_value(format!("row{i}"));
    }
    let batch = RecordBatch::try_new(Arc::new(schema), vec![Arc::new(builder.finish())]).unwrap();
    let mut buf = Vec::new();
    {
        let mut writer =
            arrow_ipc::writer::StreamWriter::try_new(&mut buf, &batch.schema()).unwrap();
        writer.write(&batch).unwrap();
        writer.finish().unwrap();
    }
    buf
}

/// Build an IPC stream with correct metadata, 2 batches of 1 row each.
fn build_echo_ipc_two_batches() -> Vec<u8> {
    let schema = Schema::new_with_metadata(
        vec![Field::new("text", arrow_schema::DataType::Utf8, false)],
        HashMap::from([
            ("laic.skill_id".into(), "echo".into()),
            ("laic.version".into(), "1.0.0".into()),
            ("laic.direction".into(), "input".into()),
        ]),
    );
    let schema = Arc::new(schema);
    let mut buf = Vec::new();
    {
        let mut writer = arrow_ipc::writer::StreamWriter::try_new(&mut buf, &schema).unwrap();
        for _ in 0..2 {
            let mut builder = StringBuilder::new();
            builder.append_value("hello");
            let batch = RecordBatch::try_new(Arc::clone(&schema), vec![Arc::new(builder.finish())])
                .unwrap();
            writer.write(&batch).unwrap();
        }
        writer.finish().unwrap();
    }
    buf
}

#[test]
fn f2_reject_zero_rows() {
    let bytes = build_echo_ipc_n_rows(0);
    let err = echo_laic::EchoInput::from_arrow_ipc(&bytes)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("cardinality error") && err.contains("0 rows"),
        "expected 0-row cardinality error, got: {err}"
    );
}

#[test]
fn f2_reject_multiple_rows() {
    let bytes = build_echo_ipc_n_rows(3);
    let err = echo_laic::EchoInput::from_arrow_ipc(&bytes)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("cardinality error") && err.contains("3 rows"),
        "expected multi-row cardinality error, got: {err}"
    );
}

#[test]
fn f2_reject_two_batches() {
    let bytes = build_echo_ipc_two_batches();
    let err = echo_laic::EchoInput::from_arrow_ipc(&bytes)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("cardinality error") && err.contains("more than one RecordBatch"),
        "expected multi-batch cardinality error, got: {err}"
    );
}
