use std::path::PathBuf;

use laic::transport::ipc::IpcConnection;

use super::process::{required_arg, required_usize, write_ready};
use super::roundtrip::{
    drain_quic_shutdown, receive_ipc, receive_quic, serve_ipc_connection, serve_quic_connection,
};
use super::types::{
    bind_server, ensure_payload_len, ensure_shutdown, is_shutdown, make_error, sample_message,
    BenchError, PkiBytes,
};
pub(super) fn run_child(args: &[String]) -> Result<(), BenchError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let mode = args
        .first()
        .ok_or_else(|| make_error("missing child mode"))?
        .as_str();
    match mode {
        "ipc-server" => runtime.block_on(run_ipc_server(args)),
        "quic-server" => runtime.block_on(run_quic_server(args)),
        "ipc-fanout-server" => runtime.block_on(run_ipc_fanout_server(args)),
        "quic-fanout-server" => runtime.block_on(run_quic_fanout_server(args)),
        other => Err(make_error(format!("unknown child mode '{other}'"))),
    }
}

pub(super) async fn run_ipc_server(args: &[String]) -> Result<(), BenchError> {
    let name = required_arg(args, "--name")?;
    let payload_bytes = required_usize(args, "--payload-bytes")?;
    let roundtrips = required_usize(args, "--roundtrips")?;
    let ready_file = PathBuf::from(required_arg(args, "--ready-file")?);
    let mut server = IpcConnection::open_server(&name)?;
    write_ready(&ready_file, "ready")?;
    serve_ipc_connection(&mut server, payload_bytes, roundtrips).await?;
    let _ = server.close().await;
    Ok(())
}

pub(super) async fn run_quic_server(args: &[String]) -> Result<(), BenchError> {
    let payload_bytes = required_usize(args, "--payload-bytes")?;
    let roundtrips = required_usize(args, "--roundtrips")?;
    let ready_file = PathBuf::from(required_arg(args, "--ready-file")?);
    let cert_dir = PathBuf::from(required_arg(args, "--cert-dir")?);
    let pki = PkiBytes::read_from_dir(&cert_dir)?;
    let server = bind_server(&pki.server_tls())?;
    let addr = server.local_addr()?;
    write_ready(&ready_file, &addr.to_string())?;
    eprintln!("LAIC_BHOST_CHILD_STAGE quic_server_ready {addr}");
    let mut conn = server.accept().await?;
    eprintln!("LAIC_BHOST_CHILD_STAGE quic_server_accepted");
    let heartbeat = receive_quic(&mut conn, "QUIC child heartbeat").await?;
    ensure_shutdown("QUIC child heartbeat", &heartbeat)?;
    eprintln!("LAIC_BHOST_CHILD_STAGE quic_server_heartbeat");
    serve_quic_connection(&mut conn, payload_bytes, roundtrips).await?;
    eprintln!("LAIC_BHOST_CHILD_STAGE quic_server_served");
    let _ = conn.close().await;
    server.close();
    Ok(())
}

pub(super) async fn run_ipc_fanout_server(args: &[String]) -> Result<(), BenchError> {
    let prefix = required_arg(args, "--name")?;
    let clients = required_usize(args, "--clients")?;
    let payload_bytes = required_usize(args, "--payload-bytes")?;
    let roundtrips = required_usize(args, "--roundtrips")?;
    let ready_file = PathBuf::from(required_arg(args, "--ready-file")?);
    let mut servers = Vec::with_capacity(clients);
    for idx in 0..clients {
        servers.push(IpcConnection::open_server(&format!("{prefix}/{idx}"))?);
    }
    write_ready(&ready_file, "ready")?;
    let reply = sample_message(payload_bytes, 102);
    for _ in 0..roundtrips {
        for server in &mut servers {
            let request = receive_ipc(server, "IPC fan-out server receive").await?;
            if is_shutdown(&request) {
                continue;
            }
            ensure_payload_len("IPC fan-out request", &request, payload_bytes)?;
            server.send(&reply).await?;
        }
    }
    for server in &mut servers {
        let shutdown = receive_ipc(server, "IPC fan-out shutdown").await?;
        ensure_shutdown("IPC fan-out shutdown", &shutdown)?;
    }
    for server in &mut servers {
        let _ = server.close().await;
    }
    Ok(())
}

pub(super) async fn run_quic_fanout_server(args: &[String]) -> Result<(), BenchError> {
    let clients = required_usize(args, "--clients")?;
    let payload_bytes = required_usize(args, "--payload-bytes")?;
    let roundtrips = required_usize(args, "--roundtrips")?;
    let ready_file = PathBuf::from(required_arg(args, "--ready-file")?);
    let cert_dir = PathBuf::from(required_arg(args, "--cert-dir")?);
    let pki = PkiBytes::read_from_dir(&cert_dir)?;
    let server = bind_server(&pki.server_tls())?;
    let addr = server.local_addr()?;
    write_ready(&ready_file, &addr.to_string())?;
    eprintln!("LAIC_BHOST_CHILD_STAGE quic_fanout_server_ready {addr}");

    let mut conns = Vec::with_capacity(clients);
    for _ in 0..clients {
        let mut conn = server.accept().await?;
        eprintln!("LAIC_BHOST_CHILD_STAGE quic_fanout_server_accepted");
        let heartbeat = receive_quic(&mut conn, "QUIC fan-out heartbeat").await?;
        ensure_shutdown("QUIC fan-out heartbeat", &heartbeat)?;
        conns.push(conn);
    }

    let reply = sample_message(payload_bytes, 202);
    for _ in 0..roundtrips {
        for conn in &mut conns {
            let request = receive_quic(conn, "QUIC fan-out server receive").await?;
            ensure_payload_len("QUIC fan-out request", &request, payload_bytes)?;
            conn.send(&reply).await?;
        }
    }
    drain_quic_shutdown().await;
    eprintln!("LAIC_BHOST_CHILD_STAGE quic_fanout_server_served");
    for conn in &mut conns {
        let _ = conn.close().await;
    }
    server.close();
    Ok(())
}
