use criterion::{black_box, criterion_group, criterion_main, Criterion};

use arrow_array::{Float32Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
use std::sync::Arc;

use laic::codec::arrow::{decode_record_batch, encode_record_batch};

/// Benchmark Arrow IPC encode/decode with a 768-dim f32 vector
/// (typical AI embedding size).
fn arrow_768_f32(c: &mut Criterion) {
    let schema = Arc::new(Schema::new(vec![Field::new(
        "embedding",
        DataType::Float32,
        false,
    )]));
    let data: Vec<f32> = (0..768).map(|i| i as f32 * 0.001).collect();
    let array = Float32Array::from(data);
    let Ok(batch) = RecordBatch::try_new(schema, vec![Arc::new(array)]) else {
        panic!("bench setup: batch creation failed");
    };
    let Ok(encoded) = encode_record_batch(&batch) else {
        panic!("bench setup: encode failed");
    };

    c.bench_function("arrow_768f32_encode", |b| {
        b.iter(|| encode_record_batch(black_box(&batch)))
    });

    c.bench_function("arrow_768f32_decode", |b| {
        b.iter(|| decode_record_batch(black_box(&encoded)))
    });
}

criterion_group!(benches, arrow_768_f32);
criterion_main!(benches);
