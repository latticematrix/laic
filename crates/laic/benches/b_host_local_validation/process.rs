use std::env;
use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::types::{make_error, BenchError, CHILD_EXIT_TIMEOUT, CHILD_READY_TIMEOUT};
pub(super) fn spawn_child(args: &[&str]) -> Result<Child, BenchError> {
    let exe = env::current_exe()?;
    let mut command = Command::new(exe);
    command
        .arg("--laic-child")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    Ok(command.spawn()?)
}

pub(super) fn wait_ready(path: &Path) -> Result<String, BenchError> {
    let start = Instant::now();
    loop {
        if path.exists() {
            let content = fs::read_to_string(path)?.trim().to_string();
            if !content.is_empty() {
                return Ok(content);
            }
        }
        if start.elapsed() > CHILD_READY_TIMEOUT {
            return Err(make_error(format!(
                "child did not write ready file '{}'",
                path.display()
            )));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

pub(super) fn wait_child_success(mut child: Child, label: &str) -> Result<(), BenchError> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            return Err(make_error(format!(
                "{label} exited with {status}; child stderr was inherited by the benchmark process"
            )));
        }
        if start.elapsed() > CHILD_EXIT_TIMEOUT {
            let _ = child.kill();
            return Err(make_error(format!("{label} did not exit in time")));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

pub(super) fn write_ready(path: &Path, content: &str) -> Result<(), BenchError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

pub(super) fn path_arg(path: &Path) -> String {
    path.display().to_string()
}

pub(super) fn required_arg(args: &[String], name: &str) -> Result<String, BenchError> {
    for pair in args.windows(2) {
        if pair[0] == name {
            return Ok(pair[1].clone());
        }
    }
    Err(make_error(format!("missing required child arg {name}")))
}

pub(super) fn required_usize(args: &[String], name: &str) -> Result<usize, BenchError> {
    let value = required_arg(args, name)?;
    let parsed = value.parse::<usize>().map_err(|err| {
        make_error(format!(
            "invalid {name} value '{value}'; expected usize: {err}"
        ))
    })?;
    Ok(parsed)
}
