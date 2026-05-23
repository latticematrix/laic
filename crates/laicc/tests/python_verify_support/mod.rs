use std::path::{Path, PathBuf};
use std::process::Output;

#[path = "../support/python_fixture.rs"]
mod python_fixture;

pub(crate) use self::python_fixture::{
    driver_path, generated_import_prelude, python_command, python_driver_script,
    write_generated_package,
};

pub(crate) fn python_install_hint() -> &'static str {
    "python -m pip install -r crates/laicc/tests/python_runtime/requirements.txt"
}

pub(crate) fn assert_process_succeeded(output: &Output, fixture: &str) {
    // `stdout` containing PASS is not sufficient evidence; a driver can print and still
    // terminate non-zero. Keep Python verify aligned with the stricter TypeScript gate.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Python process failed for {fixture} with status {:?}\nIf dependencies are missing, run: {}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code(),
        python_install_hint()
    );
}

/// Create a fresh per-case package root so Tier 2/Tier 3 verify exercise the same layout.
pub(crate) fn fresh_case_dir(case_name: &str) -> PathBuf {
    // WHY: `python_verify` owns the `laicc_python_verify` temp namespace, but it must
    // not fork package-style fixture layout rules away from `contract_surface_compat`.
    python_fixture::fresh_case_dir(std::env::temp_dir().join("laicc_python_verify"), case_name)
}

pub(crate) const LIST_TENSOR_METADATA_REQUIRED_SCRIPT: &str = r#"
schema = pa.schema([
    pa.field("embeddings", pa.list_(pa.binary()), nullable=False),
    pa.field("primary", pa.binary(), nullable=False, metadata={
        b"laic.tensor.dtype": b"f32",
        b"laic.tensor.shape": b"[3,224,224]",
        b"laic.tensor.version": b"1",
    }),
], metadata={
    b"laic.skill_id": b"tensor-container",
    b"laic.version": b"1.0.0",
    b"laic.direction": b"input",
})

batch = pa.RecordBatch.from_pydict({
    "embeddings": [[b"\x01" * 16]],
    "primary": [b"\x02" * 16],
}, schema=schema)

sink = pa.BufferOutputStream()
writer = ipc.new_stream(sink, schema)
writer.write_batch(batch)
writer.close()
data = sink.getvalue().to_pybytes()

try:
    TensorContainerInput.from_ipc(data)
    raise AssertionError("expected list<tensor> metadata rejection")
except ValueError as exc:
    message = str(exc)
    assert "embeddings" in message

print("PASS")
"#;

pub(crate) const OPTIONAL_TENSOR_METADATA_REQUIRED_SCRIPT: &str = r#"
schema = pa.schema([
    pa.field("features", pa.binary(), nullable=True),
    pa.field("scores", pa.list_(pa.binary()), nullable=False, metadata={
        b"laic.tensor.dtype": b"f32",
        b"laic.tensor.shape": b"[10]",
        b"laic.tensor.version": b"1",
    }),
], metadata={
    b"laic.skill_id": b"tensor-container",
    b"laic.version": b"1.0.0",
    b"laic.direction": b"output",
})

batch = pa.RecordBatch.from_pydict({
    "features": [None],
    "scores": [[b"\x03" * 8]],
}, schema=schema)

sink = pa.BufferOutputStream()
writer = ipc.new_stream(sink, schema)
writer.write_batch(batch)
writer.close()
data = sink.getvalue().to_pybytes()

try:
    TensorContainerOutput.from_ipc(data)
    raise AssertionError("expected optional<tensor> metadata rejection")
except ValueError as exc:
    message = str(exc)
    assert "features" in message

print("PASS")
"#;

pub(crate) const EMBEDDING_INPUT_MODEL_TYPE_MISMATCH_SCRIPT: &str = r#"
schema = pa.schema([
    pa.field("text", pa.string(), nullable=False),
    pa.field("model", pa.int32(), nullable=False),
    pa.field("max_tokens", pa.int32(), nullable=False),
], metadata={
    b"laic.skill_id": b"text-embedding",
    b"laic.version": b"1.0.0",
    b"laic.direction": b"input",
})

batch = pa.RecordBatch.from_pydict({
    "text": ["hello"],
    "model": [7],
    "max_tokens": [512],
}, schema=schema)

sink = pa.BufferOutputStream()
writer = ipc.new_stream(sink, schema)
writer.write_batch(batch)
writer.close()
data = sink.getvalue().to_pybytes()

rejected = False
try:
    EmbeddingInput.from_ipc(data)
except Exception:
    rejected = True

if not rejected:
    raise AssertionError("expected scalar field type rejection")

print("PASS")
"#;

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    #[test]
    fn assert_process_succeeded_rejects_nonzero_exit_even_with_pass_marker() {
        let output = python_command(Path::new("."))
            .args(["-c", "print('PASS')\nraise SystemExit(1)"])
            .output()
            .unwrap_or_else(|e| panic!("python not found: {e}"));
        let result = std::panic::catch_unwind(|| assert_process_succeeded(&output, "synthetic"));
        assert!(
            result.is_err(),
            "shared process guard must reject non-zero Python exits even if stdout contains PASS"
        );
    }

    #[test]
    fn shared_python_fixture_helper_uses_package_style_layout() {
        let case_dir = fresh_case_dir("shared_package_layout");
        let module_name = write_generated_package(&case_dir, "echo", "class EchoInput: pass\n");
        let driver_path = driver_path(&case_dir);

        assert_eq!(module_name, "echo_laic");
        assert!(
            case_dir.join("generated").join("echo_laic.py").exists(),
            "shared helper must place generated Python bindings inside generated/<fixture>_laic.py"
        );
        assert!(
            case_dir.join("generated").join("__init__.py").exists(),
            "shared helper must keep the generated package importable"
        );
        assert_eq!(
            driver_path.file_name().and_then(|name| name.to_str()),
            Some("driver.py"),
            "shared helper must keep the driver path at the package root"
        );
        std::fs::remove_dir_all(&case_dir).unwrap_or_else(|e| panic!("cleanup helper test: {e}"));
    }

    #[test]
    fn shared_python_fixture_helper_centralizes_import_prelude_and_runner() {
        let script = python_driver_script("echo", "print('PASS')");
        assert!(
            script.starts_with("from generated.echo_laic import *\n\n"),
            "shared helper must own the package-style import prelude used by Python verify"
        );
        assert!(
            script.ends_with("print('PASS')\n"),
            "shared helper must preserve the caller-provided driver body"
        );

        assert_eq!(
            generated_import_prelude("echo"),
            "from generated.echo_laic import *",
            "shared helper must own the generated module naming contract"
        );

        let case_dir = Path::new("C:/tmp/laic_python_helper_case");
        let command = python_command(case_dir);
        assert_eq!(
            command.get_program().to_string_lossy(),
            "python",
            "shared helper must keep Python verify and contract-surface on the same interpreter entrypoint"
        );
        let pythonpath = command
            .get_envs()
            .find_map(|(key, value): (&OsStr, Option<&OsStr>)| {
                (key == OsStr::new("PYTHONPATH"))
                    .then(|| value.map(|item: &OsStr| item.to_string_lossy().into_owned()))
                    .flatten()
            })
            .unwrap_or_else(|| {
                panic!("shared helper must configure PYTHONPATH for package-style imports")
            });
        assert_eq!(
            pythonpath,
            case_dir.to_string_lossy(),
            "shared helper must keep PYTHONPATH aligned with the case root package layout"
        );
    }
}
