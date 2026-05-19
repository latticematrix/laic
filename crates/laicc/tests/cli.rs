//! CLI behavior tests for the laicc binary.

use std::process::Command;

fn laicc_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_laicc"))
}

#[test]
fn missing_input_file_reports_input_path() {
    let missing = "tests/fixtures/definitely_missing_input.laic";

    let output = laicc_command()
        .args([missing, "--lang", "rust", "-o", ".tmp/laicc-cli-test"])
        .output()
        .unwrap_or_else(|err| panic!("failed to run laicc: {err}"));

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to read input file"),
        "stderr should name the failed operation, got:\n{stderr}"
    );
    assert!(
        stderr.contains(missing),
        "stderr should include the missing input path, got:\n{stderr}"
    );
}

#[test]
fn invalid_language_is_rejected_before_reading_input() {
    let missing = "tests/fixtures/definitely_missing_input.laic";

    let output = laicc_command()
        .args([missing, "--lang", "ruby", "-o", ".tmp/laicc-cli-test"])
        .output()
        .unwrap_or_else(|err| panic!("failed to run laicc: {err}"));

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid value") && stderr.contains("ruby"),
        "stderr should reject the unsupported language, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("failed to read input file"),
        "unsupported --lang should be rejected before input I/O, got:\n{stderr}"
    );
}
