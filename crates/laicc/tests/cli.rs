//! CLI behavior tests for the laicc binary.

use std::fs;
use std::process::Command;

fn laicc_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_laicc"))
}

fn reset_tmp_dir(path: &str) {
    let _ = fs::remove_dir_all(path);
    fs::create_dir_all(path).unwrap_or_else(|err| panic!("failed to create {path}: {err}"));
}

fn inspect_schema_stdout(fixture: &str) -> String {
    let path = format!("tests/fixtures/{fixture}");
    let output = laicc_command()
        .args(["inspect-schema", path.as_str()])
        .output()
        .unwrap_or_else(|err| panic!("failed to run laicc: {err}"));

    assert!(
        output.status.success(),
        "inspect-schema {fixture} should succeed, stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout).into_owned()
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

#[test]
fn generation_path_still_accepts_input_lang_and_output_flags() {
    let tmp = ".tmp/laicc-cli-generation-path";
    reset_tmp_dir(tmp);

    let output = laicc_command()
        .args(["tests/fixtures/embedding.laic", "--lang", "rust", "-o", tmp])
        .output()
        .unwrap_or_else(|err| panic!("failed to run laicc: {err}"));

    assert!(
        output.status.success(),
        "generation path should succeed, stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        fs::metadata(format!("{tmp}/embedding_laic.rs")).is_ok(),
        "generation path should write embedding_laic.rs"
    );
}

#[test]
fn inspect_schema_prints_human_readable_contract_metadata() {
    let stdout = inspect_schema_stdout("embedding.laic");

    for expected in [
        "LAIC schema inspection",
        "human-readable",
        "EmbeddingInput",
        "EmbeddingOutput",
        "laic.skill_id = text-embedding",
        "laic.version = 1.0.0",
        "laic.direction = input",
        "laic.direction = output",
        "embedding",
        "tensor<f32>[768]",
        "DataType::Binary",
        "laic.tensor.dtype",
        "laic.tensor.shape",
        "laic.tensor.version",
    ] {
        assert!(
            stdout.contains(expected),
            "inspect-schema stdout should contain {expected:?}, got:\n{stdout}"
        );
    }
}

#[test]
fn inspect_schema_covers_current_structural_fixtures() {
    for (fixture, expected) in [
        (
            "list_types.laic",
            &[
                "BatchEmbedInput",
                "texts",
                "list<string>",
                "DataType::List(item: DataType::Utf8)",
                "weights",
                "list<f32>",
                "token_counts",
                "list<i32>",
            ][..],
        ),
        (
            "optional_types.laic",
            &[
                "SearchInput",
                "max_results",
                "optional<i32>",
                "Nullable: true",
                "filter_tag",
                "optional<string>",
                "next_cursor",
            ][..],
        ),
        (
            "map_types.laic",
            &[
                "MapDemoInput",
                "metadata",
                "map<string, string>",
                "DataType::Map(keys: DataType::Utf8, values: DataType::Utf8)",
                "scores",
                "map<string, f64>",
                "flags",
                "map<i32, bool>",
            ][..],
        ),
        (
            "tensor_container.laic",
            &[
                "TensorContainerInput",
                "embeddings",
                "list<tensor<f32>[768]>",
                "primary",
                "tensor<f32>[3,224,224]",
                "features",
                "optional<tensor<f64>[512]>",
                "scores",
                "list<tensor<f32>[10]>",
                "laic.tensor.dtype",
                "laic.tensor.shape",
                "laic.tensor.version",
            ][..],
        ),
    ] {
        let stdout = inspect_schema_stdout(fixture);
        for item in expected {
            assert!(
                stdout.contains(item),
                "inspect-schema {fixture} stdout should contain {item:?}, got:\n{stdout}"
            );
        }
    }
}

#[test]
fn inspect_schema_invalid_contract_uses_validation_error_path() {
    let tmp = ".tmp/laicc-cli-inspect-invalid";
    reset_tmp_dir(tmp);
    let invalid_path = format!("{tmp}/fixed_zero_tensor.laic");
    fs::write(
        &invalid_path,
        r#"
version "1.0.0";

skill invalid_tensor {
    id = "invalid-tensor";

    input InvalidTensorInput {
        embedding: tensor<f32>[0];
    }

    output InvalidTensorOutput {
        ok: bool;
    }
}
"#,
    )
    .unwrap_or_else(|err| panic!("failed to write invalid fixture: {err}"));

    let output = laicc_command()
        .args(["inspect-schema", invalid_path.as_str()])
        .output()
        .unwrap_or_else(|err| panic!("failed to run laicc: {err}"));

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("validation error") && stderr.contains("cannot use fixed dimension 0"),
        "inspect-schema should reuse validator errors, got:\n{stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("LAIC schema inspection"),
        "invalid contracts must not print a successful inspection, got:\n{stdout}"
    );
}
