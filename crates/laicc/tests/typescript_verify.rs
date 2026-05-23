//! Tier 2 (TypeScript compile) + Tier 3 (roundtrip) verification.
//!
//! Feature-gated: `cargo test --features typescript-verify`
//! Requires: Node.js + npm fixture dependencies installed.
//!
//! WHY: this harness is intentionally repo-local and reproducible. We do not depend on
//! a globally installed `tsc`, and we treat runtime failures as contract failures rather
//! than best-effort smoke tests.

#![cfg(feature = "typescript-verify")]

mod support;
mod typescript_verify_support;

use std::path::Path;
use std::process::{Command, Output};

use support::i64_default_source;
use support::nul_digit_defaults_source;
use typescript_verify_support::{
    npm_program, runtime_dir, write_compile_case, write_roundtrip_case,
};

const ECHO_ROUNDTRIP_DRIVER: &str = r#"
import { EchoInput, EchoOutput } from "./index";

const input = new EchoInput("hello world");
const inputData = input.toIpc();
const inputRestored = EchoInput.fromIpc(inputData);
if (inputRestored.text !== "hello world") {
  throw new Error(`unexpected input text: ${inputRestored.text}`);
}

const output = new EchoOutput("echo: hello");
const outputData = output.toIpc();
const outputRestored = EchoOutput.fromIpc(outputData);
if (outputRestored.text !== "echo: hello") {
  throw new Error(`unexpected output text: ${outputRestored.text}`);
}

console.log("PASS");
"#;

const EMBEDDING_ROUNDTRIP_DRIVER: &str = r#"
import { EmbeddingInput, EmbeddingOutput } from "./index";

const bytes = new Uint8Array(768 * 4);

const input = new EmbeddingInput("hello");
const inputData = input.toIpc();
const inputRestored = EmbeddingInput.fromIpc(inputData);
if (inputRestored.model !== "default") {
  throw new Error(`unexpected model: ${inputRestored.model}`);
}
if (inputRestored.max_tokens !== 512) {
  throw new Error(`unexpected max_tokens: ${inputRestored.max_tokens}`);
}

const output = new EmbeddingOutput(bytes, 42);
const outputData = output.toIpc();
const outputRestored = EmbeddingOutput.fromIpc(outputData);
if (outputRestored.token_count !== 42) {
  throw new Error(`unexpected token_count: ${outputRestored.token_count}`);
}
if (outputRestored.embedding.length !== bytes.length) {
  throw new Error(`unexpected embedding length: ${outputRestored.embedding.length}`);
}

console.log("PASS");
"#;

const OPTIONAL_TYPES_ROUNDTRIP_DRIVER: &str = r#"
import { SearchInput, SearchOutput } from "./index";

const input = new SearchInput("hello", null, null, null);
const inputData = input.toIpc();
const inputRestored = SearchInput.fromIpc(inputData);
if (inputRestored.max_results !== null) {
  throw new Error(`unexpected max_results: ${inputRestored.max_results}`);
}
if (inputRestored.filter_tag !== null) {
  throw new Error(`unexpected filter_tag: ${inputRestored.filter_tag}`);
}
if (inputRestored.threshold !== null) {
  throw new Error(`unexpected threshold: ${inputRestored.threshold}`);
}

const output = new SearchOutput(["a", "b"], 2, null);
const outputData = output.toIpc();
const outputRestored = SearchOutput.fromIpc(outputData);
if (outputRestored.next_cursor !== null) {
  throw new Error(`unexpected next_cursor: ${outputRestored.next_cursor}`);
}
if (outputRestored.total_count !== 2) {
  throw new Error(`unexpected total_count: ${outputRestored.total_count}`);
}

console.log("PASS");
"#;

const TENSOR_METADATA_MISMATCH_REJECTED_DRIVER: &str = r#"
import * as arrow from "apache-arrow";
import { EmbeddingOutput } from "./index";

const bytes = new Uint8Array(768 * 4);
const schema = new arrow.Schema([
  new arrow.Field("embedding", new arrow.Binary(), false, new Map([
    ["laic.tensor.dtype", "f64"],
    ["laic.tensor.shape", "[768]"],
    ["laic.tensor.version", "1"],
  ])),
  new arrow.Field("token_count", new arrow.Int32(), false),
], new Map([
  ["laic.skill_id", "text-embedding"],
  ["laic.version", "1.0.0"],
  ["laic.direction", "output"],
]));

const columns: Record<string, arrow.Vector> = {
  embedding: arrow.vectorFromArray([bytes], schema.fields[0]!.type),
  token_count: arrow.vectorFromArray([42], schema.fields[1]!.type),
};

const table = new arrow.Table(schema, columns as any);
const ipc = arrow.tableToIPC(table);

try {
  EmbeddingOutput.fromIpc(ipc);
  throw new Error("expected tensor metadata mismatch");
} catch (error) {
  const message = String((error as Error).message ?? error);
  if (!message.includes("laic.tensor.dtype") || !message.includes("expected 'f32'")) {
    throw error;
  }
}

console.log("PASS");
"#;

const TRAILING_EMPTY_RECORD_BATCH_REJECTED_DRIVER: &str = r#"
import * as arrow from "apache-arrow";
import { EchoInput } from "./index";

const original = new EchoInput("hello world");
const originalTable = arrow.tableFromIPC(original.toIpc());
const firstBatch = originalTable.batches[0]!;
const trailingEmptyBatch = firstBatch.slice(1, 1);
const driftedTable = new arrow.Table(originalTable.schema, [firstBatch, trailingEmptyBatch]);
const driftedIpc = arrow.tableToIPC(driftedTable);

try {
  EchoInput.fromIpc(driftedIpc);
  throw new Error("expected trailing RecordBatch rejection");
} catch (error) {
  const message = String((error as Error).message ?? error);
  if (!message.includes("more than one RecordBatch")) {
    throw error;
  }
}

console.log("PASS");
"#;

const EMBEDDING_DEFAULT_FIELD_TYPE_MISMATCH_REJECTED_DRIVER: &str = r#"
import * as arrow from "apache-arrow";
import { EmbeddingInput } from "./index";

const schema = new arrow.Schema([
  new arrow.Field("text", new arrow.Utf8(), false),
  new arrow.Field("model", new arrow.Int32(), false),
  new arrow.Field("max_tokens", new arrow.Int32(), false),
], new Map([
  ["laic.skill_id", "text-embedding"],
  ["laic.version", "1.0.0"],
  ["laic.direction", "input"],
]));

const columns: Record<string, arrow.Vector> = {
  text: arrow.vectorFromArray(["hello"], schema.fields[0]!.type),
  model: arrow.vectorFromArray([7], schema.fields[1]!.type),
  max_tokens: arrow.vectorFromArray([512], schema.fields[2]!.type),
};

const table = new arrow.Table(schema, columns as any);
const ipc = arrow.tableToIPC(table);

let rejected = false;
try {
  EmbeddingInput.fromIpc(ipc);
} catch {
  rejected = true;
}

if (!rejected) {
  throw new Error("expected scalar field type rejection");
}

console.log("PASS");
"#;

fn compile_and_generate(source: &str) -> String {
    let file = laicc::compile(source).unwrap_or_else(|e| panic!("compile failed: {e}"));
    laicc::generate_typescript(&file).unwrap_or_else(|e| panic!("generate_typescript failed: {e}"))
}

fn compile_fixture(name: &str) -> String {
    compile_and_generate(&load_fixture(name))
}

fn load_fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read fixture {name}: {e}"))
}

fn assert_process_succeeded(output: &Output, context: &str) {
    // `stdout` containing PASS is not sufficient evidence; a driver can print and still
    // terminate non-zero. Always gate on the exit status first.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "process failed for {context} with status {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
}

#[test]
fn npm_program_matches_platform() {
    let expected = if cfg!(windows) { "npm.cmd" } else { "npm" };
    assert_eq!(npm_program(), expected);
}

fn ensure_toolchain_ready() {
    let output = Command::new(npm_program())
        .args(["exec", "tsc", "--", "--version"])
        .current_dir(runtime_dir())
        .output()
        .unwrap_or_else(|e| panic!("{} not found: {e}", npm_program()));

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "TypeScript verify fixture is not installed; run npm ci --prefix crates/laicc/tests/typescript_runtime\nstdout: {stdout}\nstderr: {stderr}"
        );
    }
}

fn verify_compile(case_dir: &Path, case_name: &str) {
    let output = Command::new(npm_program())
        .args([
            "exec",
            "tsc",
            "--",
            "--project",
            case_dir.join("tsconfig.json").to_str().unwrap_or(""),
            "--noEmit",
        ])
        .current_dir(runtime_dir())
        .output()
        .unwrap_or_else(|e| panic!("tsc not found for {case_name}: {e}"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Tier 2 compile check failed for {case_name}:\nstdout: {stdout}\nstderr: {stderr}"
    );
}

fn verify_roundtrip(case_dir: &Path, case_name: &str) {
    let compile = Command::new(npm_program())
        .args([
            "exec",
            "tsc",
            "--",
            "--project",
            case_dir.join("tsconfig.json").to_str().unwrap_or(""),
        ])
        .current_dir(runtime_dir())
        .output()
        .unwrap_or_else(|e| panic!("tsc not found for {case_name}: {e}"));

    let compile_stdout = String::from_utf8_lossy(&compile.stdout);
    let compile_stderr = String::from_utf8_lossy(&compile.stderr);
    assert!(
        compile.status.success(),
        "Tier 3 compile failed for {case_name}:\nstdout: {compile_stdout}\nstderr: {compile_stderr}"
    );

    let run = Command::new("node")
        .arg(case_dir.join("dist").join("driver.js"))
        .output()
        .unwrap_or_else(|e| panic!("node not found for {case_name}: {e}"));
    assert_process_succeeded(&run, case_name);
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("PASS"),
        "Tier 3 roundtrip failed for {case_name}: stdout did not contain PASS\nstdout: {stdout}"
    );
}

#[test]
fn compile_all_structural_fixtures() {
    ensure_toolchain_ready();
    for fixture in [
        "echo.laic",
        "embedding.laic",
        "list_types.laic",
        "map_types.laic",
        "optional_types.laic",
        "tensor_container.laic",
        "image_classify.laic",
        "errors_only.laic",
        "multi_skill.laic",
    ] {
        let case_name = fixture.trim_end_matches(".laic");
        let case_dir = write_compile_case(case_name, &compile_fixture(fixture));
        verify_compile(&case_dir, case_name);
    }
}

#[test]
fn multiline_string_defaults_compile() {
    ensure_toolchain_ready();
    let case_dir = write_compile_case(
        "multiline_string_defaults_compile",
        &compile_fixture("escaped_defaults.laic"),
    );
    verify_compile(&case_dir, "multiline_string_defaults_compile");
}

#[test]
fn nul_followed_by_digit_defaults_compile() {
    ensure_toolchain_ready();
    let case_dir = write_compile_case(
        "nul_followed_by_digit_defaults_compile",
        &compile_and_generate(&nul_digit_defaults_source()),
    );
    verify_compile(&case_dir, "nul_followed_by_digit_defaults_compile");
}

#[test]
fn i64_defaults_compile_as_bigint_literals() {
    ensure_toolchain_ready();
    let case_dir = write_compile_case(
        "i64_defaults_compile_as_bigint_literals",
        &compile_and_generate(&i64_default_source()),
    );
    verify_compile(&case_dir, "i64_defaults_compile_as_bigint_literals");
}

#[test]
fn echo_roundtrip() {
    ensure_toolchain_ready();
    let case_dir = write_roundtrip_case(
        "echo_roundtrip",
        &compile_fixture("echo.laic"),
        ECHO_ROUNDTRIP_DRIVER,
    );
    verify_roundtrip(&case_dir, "echo_roundtrip");
}

#[test]
fn embedding_roundtrip() {
    ensure_toolchain_ready();
    let case_dir = write_roundtrip_case(
        "embedding_roundtrip",
        &compile_fixture("embedding.laic"),
        EMBEDDING_ROUNDTRIP_DRIVER,
    );
    verify_roundtrip(&case_dir, "embedding_roundtrip");
}

#[test]
fn optional_types_roundtrip() {
    ensure_toolchain_ready();
    let case_dir = write_roundtrip_case(
        "optional_types_roundtrip",
        &compile_fixture("optional_types.laic"),
        OPTIONAL_TYPES_ROUNDTRIP_DRIVER,
    );
    verify_roundtrip(&case_dir, "optional_types_roundtrip");
}

#[test]
fn tensor_metadata_mismatch_rejected() {
    ensure_toolchain_ready();
    // This negative test exists to lock in the Phase 7B fix: TS must reject tensor
    // metadata drift instead of silently accepting raw bytes with the wrong contract.
    let case_dir = write_roundtrip_case(
        "tensor_metadata_mismatch_rejected",
        &compile_fixture("embedding.laic"),
        TENSOR_METADATA_MISMATCH_REJECTED_DRIVER,
    );
    verify_roundtrip(&case_dir, "tensor_metadata_mismatch_rejected");
}

#[test]
fn trailing_empty_record_batch_rejected() {
    ensure_toolchain_ready();
    // Lock the integrated audit finding: TS must reject streams that contain more
    // than one RecordBatch even when the total logical row count still equals 1.
    let case_dir = write_roundtrip_case(
        "trailing_empty_record_batch_rejected",
        &compile_fixture("echo.laic"),
        TRAILING_EMPTY_RECORD_BATCH_REJECTED_DRIVER,
    );
    verify_roundtrip(&case_dir, "trailing_empty_record_batch_rejected");
}

#[test]
fn embedding_default_field_type_mismatch_rejected() {
    ensure_toolchain_ready();
    let case_dir = write_roundtrip_case(
        "embedding_default_field_type_mismatch_rejected",
        &compile_fixture("embedding.laic"),
        EMBEDDING_DEFAULT_FIELD_TYPE_MISMATCH_REJECTED_DRIVER,
    );
    verify_roundtrip(&case_dir, "embedding_default_field_type_mismatch_rejected");
}

#[cfg(test)]
mod helper_tests {
    use super::*;

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;

    #[test]
    fn assert_process_succeeded_rejects_nonzero_status_even_with_pass_marker() {
        let output = Output {
            status: exit_status_from_code(1),
            stdout: b"PASS\n".to_vec(),
            stderr: b"boom\n".to_vec(),
        };

        let result =
            std::panic::catch_unwind(|| assert_process_succeeded(&output, "synthetic_failure"));
        assert!(
            result.is_err(),
            "expected non-zero process status to be rejected"
        );
    }

    #[test]
    fn shared_typescript_fixture_helper_uses_checked_in_package_root_layout() {
        let case_dir = typescript_verify_support::write_package_root_case(
            ".helper-tests",
            "shared_package_layout",
            "export const generated = 1;\n",
            Some("console.log(\"PASS\");\n"),
        );

        let checked_in_index = std::fs::read_to_string(
            typescript_verify_support::runtime_dir()
                .join("src")
                .join("index.ts"),
        )
        .unwrap_or_else(|e| panic!("read checked-in index.ts: {e}"));
        let generated_index = std::fs::read_to_string(case_dir.join("src").join("index.ts"))
            .unwrap_or_else(|e| panic!("read generated index.ts: {e}"));
        assert_eq!(
            generated_index, checked_in_index,
            "shared helper must reuse the checked-in package-root index.ts"
        );

        let tsconfig = std::fs::read_to_string(case_dir.join("tsconfig.json"))
            .unwrap_or_else(|e| panic!("read generated tsconfig: {e}"));
        assert!(
            tsconfig.contains("./src/driver.ts"),
            "shared helper must include driver.ts when a roundtrip driver is requested"
        );

        typescript_verify_support::cleanup_case_dir(&case_dir);
    }

    #[cfg(unix)]
    fn exit_status_from_code(code: i32) -> std::process::ExitStatus {
        std::process::ExitStatus::from_raw(code)
    }

    #[cfg(windows)]
    fn exit_status_from_code(code: i32) -> std::process::ExitStatus {
        std::process::ExitStatus::from_raw(code as u32)
    }
}
