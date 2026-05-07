use std::env;
use std::error::Error;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use laic::transport::quic::QuicServer;
use laic::transport::tls::{ClientTlsConfig, ServerTlsConfig};
use laic::{Message, MsgType, PayloadFormat, Qos};
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

pub(super) type BenchError = Box<dyn Error + Send + Sync>;
pub(super) const OP_TIMEOUT: Duration = Duration::from_secs(10);
pub(super) const CHILD_READY_TIMEOUT: Duration = Duration::from_secs(15);
pub(super) const CHILD_EXIT_TIMEOUT: Duration = Duration::from_secs(15);
static NAME_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub(super) struct Settings {
    pub(super) payload_bytes: usize,
    pub(super) warmup_count: usize,
    pub(super) run_count: usize,
    pub(super) fanout_clients: usize,
    pub(super) fanout_run_count: usize,
    pub(super) soak_seconds: u64,
    pub(super) cases: Vec<ValidationCase>,
    pub(super) investigation_mode: bool,
    pub(super) cross_process_ipc_p95_threshold_us: f64,
    pub(super) same_host_quic_p95_threshold_us: f64,
    pub(super) fanout_ipc_p95_threshold_us: f64,
    pub(super) fanout_quic_p95_threshold_us: f64,
    pub(super) soak_p95_threshold_us: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ValidationCase {
    CrossProcessIpc,
    SameHostQuic,
    FanoutIpc,
    FanoutQuic,
    LocalSoakIpc,
    LocalSoakQuic,
}

impl ValidationCase {
    const ALL: [Self; 6] = [
        Self::CrossProcessIpc,
        Self::SameHostQuic,
        Self::FanoutIpc,
        Self::FanoutQuic,
        Self::LocalSoakIpc,
        Self::LocalSoakQuic,
    ];

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::CrossProcessIpc => "cross_process_ipc_roundtrip",
            Self::SameHostQuic => "same_host_quic_roundtrip",
            Self::FanoutIpc => "fanout_ipc_round_robin",
            Self::FanoutQuic => "fanout_quic_round_robin",
            Self::LocalSoakIpc => "local_soak_ipc",
            Self::LocalSoakQuic => "local_soak_quic",
        }
    }
}

pub(super) struct ValidationRow {
    pub(super) slice: &'static str,
    pub(super) case: &'static str,
    pub(super) path: &'static str,
    pub(super) metric: &'static str,
    pub(super) expected: String,
    pub(super) observed: String,
    pub(super) setup_us: String,
    pub(super) total_us: String,
    pub(super) p50_us: String,
    pub(super) p95_us: String,
    pub(super) p99_us: String,
    pub(super) messages_per_sec: String,
    pub(super) bytes_per_sec: String,
    pub(super) duration_ms: String,
    pub(super) status: &'static str,
    pub(super) detail: String,
}

pub(super) struct RowMeta {
    pub(super) slice: &'static str,
    pub(super) case: &'static str,
    pub(super) path: &'static str,
    pub(super) metric: &'static str,
    pub(super) expected: String,
    pub(super) threshold_us: f64,
    pub(super) payload_bytes: usize,
    pub(super) measured_roundtrips: usize,
}

pub(super) struct PkiBytes {
    pub(super) ca_cert: Vec<u8>,
    pub(super) server_cert: Vec<u8>,
    pub(super) server_key: Vec<u8>,
    pub(super) client_cert: Vec<u8>,
    pub(super) client_key: Vec<u8>,
}

pub(super) fn parse_case_filter() -> Result<Vec<ValidationCase>, BenchError> {
    let Ok(raw) = env::var("LAIC_WINDOWS_LOCAL_CASE_FILTER") else {
        return Ok(ValidationCase::ALL.to_vec());
    };
    if raw.trim().is_empty() {
        return Ok(ValidationCase::ALL.to_vec());
    }

    let mut cases = Vec::new();
    for item in raw
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let case = match item {
            "cross_process_ipc_roundtrip" => ValidationCase::CrossProcessIpc,
            "same_host_quic_roundtrip" => ValidationCase::SameHostQuic,
            "fanout_ipc_round_robin" => ValidationCase::FanoutIpc,
            "fanout_quic_round_robin" => ValidationCase::FanoutQuic,
            "local_soak_ipc" => ValidationCase::LocalSoakIpc,
            "local_soak_quic" => ValidationCase::LocalSoakQuic,
            other => {
                return Err(make_error(format!(
                    "unknown LAIC_WINDOWS_LOCAL_CASE_FILTER case '{other}'"
                )));
            }
        };
        if !cases.contains(&case) {
            cases.push(case);
        }
    }

    if cases.is_empty() {
        return Err(make_error(
            "LAIC_WINDOWS_LOCAL_CASE_FILTER did not select any cases",
        ));
    }
    Ok(cases)
}

impl Settings {
    pub(super) fn from_env() -> Result<Self, BenchError> {
        Ok(Self {
            payload_bytes: env_usize("LAIC_WINDOWS_LOCAL_PAYLOAD_BYTES", 1024)?,
            warmup_count: env_usize("LAIC_WINDOWS_LOCAL_WARMUP", 10)?,
            run_count: env_usize("LAIC_WINDOWS_LOCAL_RUNS", 100)?,
            fanout_clients: env_usize("LAIC_WINDOWS_LOCAL_FANOUT_CLIENTS", 4)?,
            fanout_run_count: env_usize("LAIC_WINDOWS_LOCAL_FANOUT_RUNS", 50)?,
            soak_seconds: env_u64("LAIC_WINDOWS_LOCAL_SOAK_SECONDS", 120)?,
            cases: parse_case_filter()?,
            investigation_mode: env::var("LAIC_WINDOWS_LOCAL_CASE_FILTER")
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false),
            cross_process_ipc_p95_threshold_us: env_f64("LAIC_WINDOWS_LOCAL_IPC_P95_US", 50_000.0)?,
            same_host_quic_p95_threshold_us: env_f64("LAIC_WINDOWS_LOCAL_QUIC_P95_US", 10_000.0)?,
            fanout_ipc_p95_threshold_us: env_f64("LAIC_WINDOWS_LOCAL_FANOUT_IPC_P95_US", 50_000.0)?,
            fanout_quic_p95_threshold_us: env_f64(
                "LAIC_WINDOWS_LOCAL_FANOUT_QUIC_P95_US",
                20_000.0,
            )?,
            soak_p95_threshold_us: env_f64("LAIC_WINDOWS_LOCAL_SOAK_P95_US", 50_000.0)?,
        })
    }
}

pub(super) fn env_usize(name: &str, default: usize) -> Result<usize, BenchError> {
    let Some(raw) = env::var_os(name) else {
        return Ok(default);
    };
    let raw = raw.to_string_lossy();
    let parsed = raw.parse::<usize>().map_err(|err| {
        make_error(format!(
            "invalid {name} value '{raw}'; expected positive integer: {err}"
        ))
    })?;
    if parsed == 0 {
        return Err(make_error(format!(
            "invalid {name} value '{raw}'; expected positive integer"
        )));
    }
    Ok(parsed)
}

pub(super) fn env_u64(name: &str, default: u64) -> Result<u64, BenchError> {
    let Some(raw) = env::var_os(name) else {
        return Ok(default);
    };
    let raw = raw.to_string_lossy();
    let parsed = raw.parse::<u64>().map_err(|err| {
        make_error(format!(
            "invalid {name} value '{raw}'; expected positive integer: {err}"
        ))
    })?;
    if parsed == 0 {
        return Err(make_error(format!(
            "invalid {name} value '{raw}'; expected positive integer"
        )));
    }
    Ok(parsed)
}

pub(super) fn env_f64(name: &str, default: f64) -> Result<f64, BenchError> {
    let Some(raw) = env::var_os(name) else {
        return Ok(default);
    };
    let raw = raw.to_string_lossy();
    let parsed = raw
        .parse::<f64>()
        .map_err(|err| make_error(format!("invalid {name} value '{raw}': {err}")))?;
    if !parsed.is_finite() || parsed <= 0.0 {
        return Err(make_error(format!(
            "invalid {name} value '{raw}'; expected positive finite number"
        )));
    }
    Ok(parsed)
}

pub(super) fn sample_message(payload_bytes: usize, msg_id: u64) -> Message {
    let payload = (0..payload_bytes)
        .map(|idx| (idx % 251) as u8)
        .collect::<Vec<_>>();
    Message::new(
        MsgType::DATA,
        msg_id,
        PayloadFormat::Raw,
        Qos::Normal,
        payload,
    )
}

pub(super) fn shutdown_message() -> Message {
    // WHY: benchmark child processes need an in-band, dependency-free stop
    // signal for duration-based soak. A zero-length msg_id=0 frame is not
    // used by measured payload rows, so it is safe as a harness sentinel.
    Message::new(
        MsgType::DATA,
        0,
        PayloadFormat::Raw,
        Qos::Normal,
        Vec::new(),
    )
}

pub(super) fn is_shutdown(message: &Message) -> bool {
    message.header().msg_id == 0 && message.payload().is_empty()
}

pub(super) fn ensure_shutdown(label: &str, message: &Message) -> Result<(), BenchError> {
    if is_shutdown(message) {
        return Ok(());
    }
    Err(make_error(format!("{label} was not a shutdown sentinel")))
}

pub(super) fn ensure_payload_len(
    label: &str,
    message: &Message,
    expected: usize,
) -> Result<(), BenchError> {
    let actual = message.payload().len();
    if actual != expected {
        return Err(make_error(format!(
            "{label} payload length mismatch: expected {expected}, got {actual}"
        )));
    }
    Ok(())
}

pub(super) fn duration_us(duration: &Duration) -> f64 {
    duration.as_secs_f64() * 1_000_000.0
}

pub(super) fn unique_name(label: &str) -> String {
    format!(
        "bhost/{}/{}/{}",
        std::process::id(),
        NAME_COUNTER.fetch_add(1, Ordering::Relaxed),
        label
    )
}

pub(super) fn bind_server(tls: &ServerTlsConfig) -> Result<QuicServer, BenchError> {
    let addr: SocketAddr = "127.0.0.1:0".parse()?;
    Ok(QuicServer::bind(addr, tls)?)
}

impl PkiBytes {
    pub(super) fn generate() -> Result<Self, BenchError> {
        let mut ca_params = CertificateParams::new(Vec::<String>::new())?;
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "LAIC B Host Bench CA");
        let ca_key = KeyPair::generate()?;
        let ca_cert = ca_params.self_signed(&ca_key)?;

        let server_params = CertificateParams::new(vec!["localhost".to_string()])?;
        let server_key = KeyPair::generate()?;
        let server_cert = server_params.signed_by(&server_key, &ca_cert, &ca_key)?;

        let client_params = CertificateParams::new(vec!["laic-bhost-client".to_string()])?;
        let client_key = KeyPair::generate()?;
        let client_cert = client_params.signed_by(&client_key, &ca_cert, &ca_key)?;

        Ok(Self {
            ca_cert: ca_cert.der().to_vec(),
            server_cert: server_cert.der().to_vec(),
            server_key: server_key.serialize_der(),
            client_cert: client_cert.der().to_vec(),
            client_key: client_key.serialize_der(),
        })
    }

    pub(super) fn write_to_dir(&self, dir: &Path) -> Result<(), BenchError> {
        fs::create_dir_all(dir)?;
        fs::write(dir.join("ca.der"), &self.ca_cert)?;
        fs::write(dir.join("server.der"), &self.server_cert)?;
        fs::write(dir.join("server.key"), &self.server_key)?;
        fs::write(dir.join("client.der"), &self.client_cert)?;
        fs::write(dir.join("client.key"), &self.client_key)?;
        Ok(())
    }

    pub(super) fn read_from_dir(dir: &Path) -> Result<Self, BenchError> {
        Ok(Self {
            ca_cert: fs::read(dir.join("ca.der"))?,
            server_cert: fs::read(dir.join("server.der"))?,
            server_key: fs::read(dir.join("server.key"))?,
            client_cert: fs::read(dir.join("client.der"))?,
            client_key: fs::read(dir.join("client.key"))?,
        })
    }

    pub(super) fn server_tls(&self) -> ServerTlsConfig {
        ServerTlsConfig::new(
            vec![CertificateDer::from(self.server_cert.clone())],
            PrivateKeyDer::from(PrivatePkcs8KeyDer::from(self.server_key.clone())),
            CertificateDer::from(self.ca_cert.clone()),
        )
    }

    pub(super) fn client_tls(&self) -> ClientTlsConfig {
        ClientTlsConfig::new(
            CertificateDer::from(self.ca_cert.clone()),
            vec![CertificateDer::from(self.client_cert.clone())],
            PrivateKeyDer::from(PrivatePkcs8KeyDer::from(self.client_key.clone())),
        )
    }
}

pub(super) struct TempDir {
    pub(super) path: PathBuf,
}

impl TempDir {
    pub(super) fn new(label: &str) -> Result<Self, BenchError> {
        let path = env::temp_dir().join(format!(
            "laic-bhost-{}-{}-{label}",
            std::process::id(),
            NAME_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(super) fn make_error(message: impl Into<String>) -> BenchError {
    std::io::Error::other(message.into()).into()
}
