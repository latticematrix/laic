use std::time::{Duration, Instant};

use laic::transport::ipc::IpcConnection;
use laic::transport::quic::QuicConnection;
use laic::Message;

use super::types::{
    duration_us, ensure_payload_len, ensure_shutdown, is_shutdown, make_error, sample_message,
    BenchError, RowMeta, Settings, ValidationRow, OP_TIMEOUT,
};
pub(super) async fn collect_ipc_roundtrips(
    client: &mut IpcConnection,
    request: &Message,
    count: usize,
) -> Result<(Vec<Duration>, Duration), BenchError> {
    let start = Instant::now();
    let mut samples = Vec::with_capacity(count);
    for _ in 0..count {
        samples.push(ipc_client_roundtrip(client, request).await?);
    }
    Ok((samples, start.elapsed()))
}

pub(super) async fn collect_quic_roundtrips(
    client: &mut QuicConnection,
    request: &Message,
    count: usize,
) -> Result<(Vec<Duration>, Duration), BenchError> {
    let start = Instant::now();
    let mut samples = Vec::with_capacity(count);
    for _ in 0..count {
        samples.push(quic_client_roundtrip(client, request).await?);
    }
    Ok((samples, start.elapsed()))
}

pub(super) async fn soak_ipc_roundtrips(
    client: &mut IpcConnection,
    request: &Message,
    seconds: u64,
) -> Result<(Vec<Duration>, Duration), BenchError> {
    let start = Instant::now();
    let target = Duration::from_secs(seconds);
    let mut samples = Vec::new();
    while start.elapsed() < target {
        samples.push(ipc_client_roundtrip(client, request).await?);
    }
    Ok((samples, start.elapsed()))
}

pub(super) async fn soak_quic_roundtrips(
    client: &mut QuicConnection,
    request: &Message,
    seconds: u64,
) -> Result<(Vec<Duration>, Duration), BenchError> {
    let start = Instant::now();
    let target = Duration::from_secs(seconds);
    let mut samples = Vec::new();
    while start.elapsed() < target {
        samples.push(quic_client_roundtrip(client, request).await?);
    }
    Ok((samples, start.elapsed()))
}

pub(super) async fn ipc_client_roundtrip(
    client: &mut IpcConnection,
    request: &Message,
) -> Result<Duration, BenchError> {
    let start = Instant::now();
    client.send(request).await?;
    let reply = receive_ipc(client, "IPC client reply").await?;
    ensure_payload_len("IPC reply", &reply, request.payload().len())?;
    Ok(start.elapsed())
}

pub(super) async fn quic_client_roundtrip(
    client: &mut QuicConnection,
    request: &Message,
) -> Result<Duration, BenchError> {
    let start = Instant::now();
    client.send(request).await?;
    let reply = receive_quic(client, "QUIC client reply").await?;
    ensure_payload_len("QUIC reply", &reply, request.payload().len())?;
    Ok(start.elapsed())
}

pub(super) async fn drain_quic_shutdown() {
    // WHY: Quinn's close is intentionally fire-and-forget in LAIC today.
    // A short harness-only drain keeps the cross-process control sentinel
    // from racing with CONNECTION_CLOSE on Windows localhost tests.
    tokio::time::sleep(Duration::from_millis(500)).await;
}

pub(super) fn row_from_samples(
    meta: RowMeta,
    setup: Duration,
    total: Duration,
    samples: Vec<Duration>,
    detail: String,
) -> ValidationRow {
    let mut samples_us = samples.iter().map(duration_us).collect::<Vec<_>>();
    samples_us.sort_by(f64::total_cmp);
    let p50 = percentile(&samples_us, 50.0);
    let p95 = percentile(&samples_us, 95.0);
    let p99 = percentile(&samples_us, 99.0);
    let messages_per_sec = (meta.measured_roundtrips * 2) as f64 / total.as_secs_f64();
    let bytes_per_sec =
        (meta.measured_roundtrips * meta.payload_bytes * 2) as f64 / total.as_secs_f64();
    let pass = p95 <= meta.threshold_us && !samples_us.is_empty();

    ValidationRow {
        slice: meta.slice,
        case: meta.case,
        path: meta.path,
        metric: meta.metric,
        expected: meta.expected,
        observed: format!(
            "p95={p95:.3};roundtrips={};errors=0",
            meta.measured_roundtrips
        ),
        setup_us: format!("{:.3}", duration_us(&setup)),
        total_us: format!("{:.3}", duration_us(&total)),
        p50_us: format!("{p50:.3}"),
        p95_us: format!("{p95:.3}"),
        p99_us: format!("{p99:.3}"),
        messages_per_sec: format!("{messages_per_sec:.3}"),
        bytes_per_sec: format!("{bytes_per_sec:.3}"),
        duration_ms: format!("{:.3}", total.as_secs_f64() * 1_000.0),
        status: if pass { "PASS" } else { "FAIL" },
        detail,
    }
}

pub(super) fn mark_investigation_timing(
    settings: &Settings,
    case: &'static str,
    stage: &'static str,
    elapsed: Duration,
) {
    if settings.investigation_mode {
        eprintln!(
            "LAIC_BHOST_INVESTIGATION_TIMING case={case} stage={stage} elapsed_us={:.3}",
            duration_us(&elapsed)
        );
    }
}

fn percentile(sorted_us: &[f64], percentile: f64) -> f64 {
    if sorted_us.is_empty() {
        return f64::NAN;
    }
    let rank = ((percentile / 100.0) * sorted_us.len() as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(sorted_us.len() - 1);
    sorted_us[index]
}

pub(super) fn print_row(row: &ValidationRow) {
    println!(
        "case,{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        row.slice,
        row.case,
        row.path,
        row.metric,
        row.expected,
        row.observed,
        row.setup_us,
        row.total_us,
        row.p50_us,
        row.p95_us,
        row.p99_us,
        row.messages_per_sec,
        row.bytes_per_sec,
        row.duration_ms,
        row.status,
        row.detail
    );
}

pub(super) async fn serve_ipc_connection(
    server: &mut IpcConnection,
    payload_bytes: usize,
    roundtrips: usize,
) -> Result<(), BenchError> {
    let reply = sample_message(payload_bytes, 101);
    loop {
        let request = receive_ipc(server, "IPC child receive").await?;
        if is_shutdown(&request) {
            return Ok(());
        }
        ensure_payload_len("IPC child request", &request, payload_bytes)?;
        server.send(&reply).await?;
        if roundtrips > 0 {
            let remaining = roundtrips.saturating_sub(1);
            if remaining == 0 {
                let shutdown = receive_ipc(server, "IPC child shutdown").await?;
                ensure_shutdown("IPC child shutdown", &shutdown)?;
                return Ok(());
            }
            return serve_ipc_counted(server, payload_bytes, remaining, &reply).await;
        }
    }
}

pub(super) async fn serve_ipc_counted(
    server: &mut IpcConnection,
    payload_bytes: usize,
    remaining: usize,
    reply: &Message,
) -> Result<(), BenchError> {
    for _ in 0..remaining {
        let request = receive_ipc(server, "IPC child receive").await?;
        ensure_payload_len("IPC child request", &request, payload_bytes)?;
        server.send(reply).await?;
    }
    let shutdown = receive_ipc(server, "IPC child shutdown").await?;
    ensure_shutdown("IPC child shutdown", &shutdown)?;
    Ok(())
}

pub(super) async fn serve_quic_connection(
    server: &mut QuicConnection,
    payload_bytes: usize,
    roundtrips: usize,
) -> Result<(), BenchError> {
    let reply = sample_message(payload_bytes, 201);
    if roundtrips == 0 {
        return serve_quic_until_idle(server, payload_bytes, &reply).await;
    }
    loop {
        let request = receive_quic(server, "QUIC child receive").await?;
        if is_shutdown(&request) {
            return Ok(());
        }
        ensure_payload_len("QUIC child request", &request, payload_bytes)?;
        server.send(&reply).await?;
        if roundtrips > 0 {
            let remaining = roundtrips.saturating_sub(1);
            if remaining == 0 {
                let shutdown = receive_quic(server, "QUIC child shutdown").await?;
                ensure_shutdown("QUIC child shutdown", &shutdown)?;
                return Ok(());
            }
            return serve_quic_counted(server, payload_bytes, remaining, &reply).await;
        }
    }
}

pub(super) async fn serve_quic_until_idle(
    server: &mut QuicConnection,
    payload_bytes: usize,
    reply: &Message,
) -> Result<(), BenchError> {
    let mut served = 0usize;
    loop {
        match tokio::time::timeout(Duration::from_secs(2), server.receive()).await {
            Ok(Ok(request)) => {
                if is_shutdown(&request) {
                    return Ok(());
                }
                ensure_payload_len("QUIC child request", &request, payload_bytes)?;
                server.send(reply).await?;
                served += 1;
            }
            Ok(Err(err)) => {
                if served > 0 {
                    return Ok(());
                }
                return Err(err.into());
            }
            Err(_) => {
                if served > 0 {
                    return Ok(());
                }
                return Err(make_error(
                    "QUIC child receive timed out before soak traffic",
                ));
            }
        }
    }
}

pub(super) async fn serve_quic_counted(
    server: &mut QuicConnection,
    payload_bytes: usize,
    remaining: usize,
    reply: &Message,
) -> Result<(), BenchError> {
    for _ in 0..remaining {
        let request = receive_quic(server, "QUIC child receive").await?;
        ensure_payload_len("QUIC child request", &request, payload_bytes)?;
        server.send(reply).await?;
    }
    let shutdown = receive_quic(server, "QUIC child shutdown").await?;
    ensure_shutdown("QUIC child shutdown", &shutdown)?;
    Ok(())
}

pub(super) fn open_ipc_clients(
    prefix: &str,
    count: usize,
) -> Result<Vec<IpcConnection>, BenchError> {
    let mut clients = Vec::with_capacity(count);
    for idx in 0..count {
        clients.push(IpcConnection::open_client(&format!("{prefix}/{idx}"))?);
    }
    Ok(clients)
}

pub(super) async fn receive_ipc(
    conn: &mut IpcConnection,
    label: &'static str,
) -> Result<Message, BenchError> {
    Ok(tokio::time::timeout(OP_TIMEOUT, conn.receive())
        .await
        .map_err(|_| make_error(format!("{label} timed out")))??)
}

pub(super) async fn receive_quic(
    conn: &mut QuicConnection,
    label: &'static str,
) -> Result<Message, BenchError> {
    Ok(tokio::time::timeout(OP_TIMEOUT, conn.receive())
        .await
        .map_err(|_| make_error(format!("{label} timed out")))??)
}
