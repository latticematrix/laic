use std::net::SocketAddr;
use std::time::Instant;

use laic::transport::ipc::IpcConnection;
use laic::transport::quic::QuicConnection;

use super::process::{path_arg, spawn_child, wait_child_success, wait_ready};
use super::roundtrip::{
    collect_ipc_roundtrips, collect_quic_roundtrips, drain_quic_shutdown, ipc_client_roundtrip,
    mark_investigation_timing, open_ipc_clients, quic_client_roundtrip, row_from_samples,
    soak_ipc_roundtrips, soak_quic_roundtrips,
};
use super::types::{
    sample_message, shutdown_message, unique_name, BenchError, PkiBytes, RowMeta, Settings,
    TempDir, ValidationRow,
};
pub(super) async fn measure_cross_process_ipc(
    settings: &Settings,
) -> Result<ValidationRow, BenchError> {
    let name = unique_name("cross-process-ipc");
    let temp = TempDir::new("cross-process-ipc")?;
    let ready_file = temp.path.join("ready.txt");
    let total_roundtrips = settings.warmup_count + settings.run_count;
    let child_start = Instant::now();
    let child = spawn_child(&[
        "ipc-server",
        "--name",
        &name,
        "--payload-bytes",
        &settings.payload_bytes.to_string(),
        "--roundtrips",
        &total_roundtrips.to_string(),
        "--ready-file",
        &path_arg(&ready_file),
    ])?;
    mark_investigation_timing(
        settings,
        "cross_process_ipc_roundtrip",
        "child_spawn",
        child_start.elapsed(),
    );
    let ready_start = Instant::now();
    wait_ready(&ready_file)?;
    mark_investigation_timing(
        settings,
        "cross_process_ipc_roundtrip",
        "child_ready_wait",
        ready_start.elapsed(),
    );

    let setup_start = Instant::now();
    let mut client = IpcConnection::open_client(&name)?;
    let setup = setup_start.elapsed();
    mark_investigation_timing(settings, "cross_process_ipc_roundtrip", "setup", setup);
    let request = sample_message(settings.payload_bytes, 1);

    let warmup_start = Instant::now();
    for _ in 0..settings.warmup_count {
        ipc_client_roundtrip(&mut client, &request).await?;
    }
    mark_investigation_timing(
        settings,
        "cross_process_ipc_roundtrip",
        "warmup",
        warmup_start.elapsed(),
    );
    let (samples, total) =
        collect_ipc_roundtrips(&mut client, &request, settings.run_count).await?;
    mark_investigation_timing(settings, "cross_process_ipc_roundtrip", "measured", total);
    let shutdown_start = Instant::now();
    client.send(&shutdown_message()).await?;
    let _ = client.close().await;
    wait_child_success(child, "cross-process IPC server")?;
    mark_investigation_timing(
        settings,
        "cross_process_ipc_roundtrip",
        "shutdown_cleanup",
        shutdown_start.elapsed(),
    );

    Ok(row_from_samples(
        RowMeta {
            slice: "cross-process",
            case: "cross_process_ipc_roundtrip",
            path: "IPC child process",
            metric: "p95_roundtrip_us",
            expected: format!(
                "p95<={:.0};errors=0",
                settings.cross_process_ipc_p95_threshold_us
            ),
            threshold_us: settings.cross_process_ipc_p95_threshold_us,
            payload_bytes: settings.payload_bytes,
            measured_roundtrips: settings.run_count,
        },
        setup,
        total,
        samples,
        format!(
            "roundtrips={};payload_bytes={}",
            settings.run_count, settings.payload_bytes
        ),
    ))
}

pub(super) async fn measure_same_host_quic(
    settings: &Settings,
) -> Result<ValidationRow, BenchError> {
    let temp = TempDir::new("same-host-quic")?;
    let cert_dir = temp.path.join("pki");
    let ready_file = temp.path.join("ready.txt");
    let pki = PkiBytes::generate()?;
    pki.write_to_dir(&cert_dir)?;
    let total_roundtrips = settings.warmup_count + settings.run_count;
    let child = spawn_child(&[
        "quic-server",
        "--payload-bytes",
        &settings.payload_bytes.to_string(),
        "--roundtrips",
        &total_roundtrips.to_string(),
        "--ready-file",
        &path_arg(&ready_file),
        "--cert-dir",
        &path_arg(&cert_dir),
    ])?;
    let addr = wait_ready(&ready_file)?.parse::<SocketAddr>()?;

    let setup_start = Instant::now();
    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls()).await?;
    client.send(&shutdown_message()).await?;
    let setup = setup_start.elapsed();
    let request = sample_message(settings.payload_bytes, 2);

    for _ in 0..settings.warmup_count {
        quic_client_roundtrip(&mut client, &request).await?;
    }
    let (samples, total) =
        collect_quic_roundtrips(&mut client, &request, settings.run_count).await?;
    client.send(&shutdown_message()).await?;
    drain_quic_shutdown().await;
    let _ = client.close().await;
    wait_child_success(child, "same-host QUIC server")?;

    Ok(row_from_samples(
        RowMeta {
            slice: "same-host-quic",
            case: "same_host_quic_roundtrip",
            path: "localhost QUIC child process",
            metric: "p95_roundtrip_us",
            expected: format!(
                "p95<={:.0};errors=0",
                settings.same_host_quic_p95_threshold_us
            ),
            threshold_us: settings.same_host_quic_p95_threshold_us,
            payload_bytes: settings.payload_bytes,
            measured_roundtrips: settings.run_count,
        },
        setup,
        total,
        samples,
        format!(
            "roundtrips={};payload_bytes={}",
            settings.run_count, settings.payload_bytes
        ),
    ))
}

pub(super) async fn measure_fanout_ipc(settings: &Settings) -> Result<ValidationRow, BenchError> {
    let prefix = unique_name("fanout-ipc");
    let temp = TempDir::new("fanout-ipc")?;
    let ready_file = temp.path.join("ready.txt");
    let total_per_client = settings.warmup_count + settings.fanout_run_count;
    let child_start = Instant::now();
    let child = spawn_child(&[
        "ipc-fanout-server",
        "--name",
        &prefix,
        "--clients",
        &settings.fanout_clients.to_string(),
        "--payload-bytes",
        &settings.payload_bytes.to_string(),
        "--roundtrips",
        &total_per_client.to_string(),
        "--ready-file",
        &path_arg(&ready_file),
    ])?;
    mark_investigation_timing(
        settings,
        "fanout_ipc_round_robin",
        "child_spawn",
        child_start.elapsed(),
    );
    let ready_start = Instant::now();
    wait_ready(&ready_file)?;
    mark_investigation_timing(
        settings,
        "fanout_ipc_round_robin",
        "child_ready_wait",
        ready_start.elapsed(),
    );

    let setup_start = Instant::now();
    let mut clients = open_ipc_clients(&prefix, settings.fanout_clients)?;
    let setup = setup_start.elapsed();
    mark_investigation_timing(settings, "fanout_ipc_round_robin", "setup", setup);
    let request = sample_message(settings.payload_bytes, 3);

    let warmup_start = Instant::now();
    for _ in 0..settings.warmup_count {
        for client in &mut clients {
            ipc_client_roundtrip(client, &request).await?;
        }
    }
    mark_investigation_timing(
        settings,
        "fanout_ipc_round_robin",
        "warmup",
        warmup_start.elapsed(),
    );
    let total_start = Instant::now();
    let mut samples = Vec::with_capacity(settings.fanout_clients * settings.fanout_run_count);
    for _ in 0..settings.fanout_run_count {
        for client in &mut clients {
            samples.push(ipc_client_roundtrip(client, &request).await?);
        }
    }
    let total = total_start.elapsed();
    mark_investigation_timing(settings, "fanout_ipc_round_robin", "measured", total);
    let shutdown_start = Instant::now();
    for client in &mut clients {
        client.send(&shutdown_message()).await?;
        drain_quic_shutdown().await;
        let _ = client.close().await;
    }
    wait_child_success(child, "IPC fan-out server")?;
    mark_investigation_timing(
        settings,
        "fanout_ipc_round_robin",
        "shutdown_cleanup",
        shutdown_start.elapsed(),
    );

    Ok(row_from_samples(
        RowMeta {
            slice: "fan-out",
            case: "fanout_ipc_round_robin",
            path: "IPC child process",
            metric: "p95_roundtrip_us",
            expected: format!(
                "clients={};p95<={:.0};errors=0",
                settings.fanout_clients, settings.fanout_ipc_p95_threshold_us
            ),
            threshold_us: settings.fanout_ipc_p95_threshold_us,
            payload_bytes: settings.payload_bytes,
            measured_roundtrips: settings.fanout_clients * settings.fanout_run_count,
        },
        setup,
        total,
        samples,
        format!(
            "clients={};roundtrips_per_client={};payload_bytes={}",
            settings.fanout_clients, settings.fanout_run_count, settings.payload_bytes
        ),
    ))
}

pub(super) async fn measure_fanout_quic(settings: &Settings) -> Result<ValidationRow, BenchError> {
    let temp = TempDir::new("fanout-quic")?;
    let cert_dir = temp.path.join("pki");
    let ready_file = temp.path.join("ready.txt");
    let pki = PkiBytes::generate()?;
    pki.write_to_dir(&cert_dir)?;
    let total_per_client = settings.warmup_count + settings.fanout_run_count;
    let child = spawn_child(&[
        "quic-fanout-server",
        "--clients",
        &settings.fanout_clients.to_string(),
        "--payload-bytes",
        &settings.payload_bytes.to_string(),
        "--roundtrips",
        &total_per_client.to_string(),
        "--ready-file",
        &path_arg(&ready_file),
        "--cert-dir",
        &path_arg(&cert_dir),
    ])?;
    let addr = wait_ready(&ready_file)?.parse::<SocketAddr>()?;

    let setup_start = Instant::now();
    let mut clients = Vec::with_capacity(settings.fanout_clients);
    for _ in 0..settings.fanout_clients {
        let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls()).await?;
        client.send(&shutdown_message()).await?;
        clients.push(client);
    }
    let setup = setup_start.elapsed();
    let request = sample_message(settings.payload_bytes, 4);

    for _ in 0..settings.warmup_count {
        for client in &mut clients {
            quic_client_roundtrip(client, &request).await?;
        }
    }
    let total_start = Instant::now();
    let mut samples = Vec::with_capacity(settings.fanout_clients * settings.fanout_run_count);
    for _ in 0..settings.fanout_run_count {
        for client in &mut clients {
            samples.push(quic_client_roundtrip(client, &request).await?);
        }
    }
    let total = total_start.elapsed();
    for client in &mut clients {
        let _ = client.close().await;
    }
    wait_child_success(child, "QUIC fan-out server")?;

    Ok(row_from_samples(
        RowMeta {
            slice: "fan-out",
            case: "fanout_quic_round_robin",
            path: "localhost QUIC child process",
            metric: "p95_roundtrip_us",
            expected: format!(
                "clients={};p95<={:.0};errors=0",
                settings.fanout_clients, settings.fanout_quic_p95_threshold_us
            ),
            threshold_us: settings.fanout_quic_p95_threshold_us,
            payload_bytes: settings.payload_bytes,
            measured_roundtrips: settings.fanout_clients * settings.fanout_run_count,
        },
        setup,
        total,
        samples,
        format!(
            "clients={};roundtrips_per_client={};payload_bytes={}",
            settings.fanout_clients, settings.fanout_run_count, settings.payload_bytes
        ),
    ))
}

pub(super) async fn measure_soak_ipc(settings: &Settings) -> Result<ValidationRow, BenchError> {
    let name = unique_name("soak-ipc");
    let temp = TempDir::new("soak-ipc")?;
    let ready_file = temp.path.join("ready.txt");
    let child_start = Instant::now();
    let child = spawn_child(&[
        "ipc-server",
        "--name",
        &name,
        "--payload-bytes",
        &settings.payload_bytes.to_string(),
        "--roundtrips",
        "0",
        "--ready-file",
        &path_arg(&ready_file),
    ])?;
    mark_investigation_timing(
        settings,
        "local_soak_ipc",
        "child_spawn",
        child_start.elapsed(),
    );
    let ready_start = Instant::now();
    wait_ready(&ready_file)?;
    mark_investigation_timing(
        settings,
        "local_soak_ipc",
        "child_ready_wait",
        ready_start.elapsed(),
    );

    let setup_start = Instant::now();
    let mut client = IpcConnection::open_client(&name)?;
    let setup = setup_start.elapsed();
    mark_investigation_timing(settings, "local_soak_ipc", "setup", setup);
    let request = sample_message(settings.payload_bytes, 5);
    let (samples, total) =
        soak_ipc_roundtrips(&mut client, &request, settings.soak_seconds).await?;
    mark_investigation_timing(settings, "local_soak_ipc", "measured", total);
    let shutdown_start = Instant::now();
    client.send(&shutdown_message()).await?;
    drain_quic_shutdown().await;
    let _ = client.close().await;
    wait_child_success(child, "IPC soak server")?;
    mark_investigation_timing(
        settings,
        "local_soak_ipc",
        "shutdown_cleanup",
        shutdown_start.elapsed(),
    );

    Ok(row_from_samples(
        RowMeta {
            slice: "soak",
            case: "local_soak_ipc",
            path: "IPC child process",
            metric: "duration_no_errors",
            expected: format!(
                "duration>={}s;p95<={:.0};errors=0",
                settings.soak_seconds, settings.soak_p95_threshold_us
            ),
            threshold_us: settings.soak_p95_threshold_us,
            payload_bytes: settings.payload_bytes,
            measured_roundtrips: samples.len(),
        },
        setup,
        total,
        samples,
        format!(
            "requested_seconds={};payload_bytes={}",
            settings.soak_seconds, settings.payload_bytes
        ),
    ))
}

pub(super) async fn measure_soak_quic(settings: &Settings) -> Result<ValidationRow, BenchError> {
    let temp = TempDir::new("soak-quic")?;
    let cert_dir = temp.path.join("pki");
    let ready_file = temp.path.join("ready.txt");
    let pki = PkiBytes::generate()?;
    pki.write_to_dir(&cert_dir)?;
    let child = spawn_child(&[
        "quic-server",
        "--payload-bytes",
        &settings.payload_bytes.to_string(),
        "--roundtrips",
        "0",
        "--ready-file",
        &path_arg(&ready_file),
        "--cert-dir",
        &path_arg(&cert_dir),
    ])?;
    let addr = wait_ready(&ready_file)?.parse::<SocketAddr>()?;

    let setup_start = Instant::now();
    let mut client = QuicConnection::connect(addr, "localhost", &pki.client_tls()).await?;
    client.send(&shutdown_message()).await?;
    let setup = setup_start.elapsed();
    let request = sample_message(settings.payload_bytes, 6);
    let (samples, total) =
        soak_quic_roundtrips(&mut client, &request, settings.soak_seconds).await?;
    eprintln!(
        "LAIC_WINDOWS_LOCAL_CASE_DETAIL local_soak_quic_roundtrips {}",
        samples.len()
    );
    client.send(&shutdown_message()).await?;
    let _ = client.close().await;
    wait_child_success(child, "QUIC soak server")?;

    Ok(row_from_samples(
        RowMeta {
            slice: "soak",
            case: "local_soak_quic",
            path: "localhost QUIC child process",
            metric: "duration_no_errors",
            expected: format!(
                "duration>={}s;p95<={:.0};errors=0",
                settings.soak_seconds, settings.soak_p95_threshold_us
            ),
            threshold_us: settings.soak_p95_threshold_us,
            payload_bytes: settings.payload_bytes,
            measured_roundtrips: samples.len(),
        },
        setup,
        total,
        samples,
        format!(
            "requested_seconds={};payload_bytes={}",
            settings.soak_seconds, settings.payload_bytes
        ),
    ))
}
