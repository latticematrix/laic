//! Tier 1: structural validation of Python codegen output (zero dependencies).
//!
//! For each fixture, verify generated Python code contains expected structural elements.
//! These tests run on every `cargo test` - no Python needed.

mod support;

use support::nul_digit_defaults_source;

fn compile_and_generate(source: &str) -> String {
    let file = laicc::compile(source).unwrap_or_else(|e| panic!("compile failed: {e}"));
    laicc::generate_python(&file).unwrap_or_else(|e| panic!("generate_python failed: {e}"))
}

fn load_fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read fixture {name}: {e}"))
}

// -- F1 regression guard: no empty f-string expressions --
fn assert_no_empty_fstring(code: &str, fixture: &str) {
    assert!(
        !code.contains("f\"{}\""),
        "F1 regression: empty f-string found in {fixture}"
    );
}

// -- F3 regression guard: no .to_pylist() --
fn assert_no_to_pylist(code: &str, fixture: &str) {
    assert!(
        !code.contains(".to_pylist()"),
        "F3 regression: .to_pylist() found in {fixture}"
    );
}

/// Common structural assertions for every generated contract.
fn assert_common_structure(code: &str, fixture: &str) {
    assert!(
        code.contains("import pyarrow as pa"),
        "{fixture}: missing pyarrow import"
    );
    assert!(
        code.contains("import pyarrow.ipc as ipc"),
        "{fixture}: missing ipc import"
    );
    assert!(
        code.contains("@dataclasses.dataclass"),
        "{fixture}: missing @dataclasses.dataclass"
    );
    assert!(
        code.contains("import dataclasses"),
        "{fixture}: missing dataclasses import"
    );
    assert_no_empty_fstring(code, fixture);
    assert_no_to_pylist(code, fixture);
}

#[test]
fn echo_structure() {
    let code = compile_and_generate(&load_fixture("echo.laic"));
    assert_common_structure(&code, "echo");

    assert!(code.contains("class EchoInput:"), "missing EchoInput class");
    assert!(
        code.contains("class EchoOutput:"),
        "missing EchoOutput class"
    );
    assert!(
        code.contains("SKILL_ID: ClassVar[str] = \"echo\""),
        "missing SKILL_ID"
    );
    assert!(
        code.contains("VERSION: ClassVar[str] = \"1.0.0\""),
        "missing VERSION"
    );
    assert!(
        code.contains("DIRECTION: ClassVar[str] = \"input\""),
        "missing input DIRECTION"
    );
    assert!(
        code.contains("DIRECTION: ClassVar[str] = \"output\""),
        "missing output DIRECTION"
    );
    assert!(
        code.contains("def to_ipc(self) -> bytes:"),
        "missing to_ipc"
    );
    assert!(
        code.contains("def from_ipc(cls, data: bytes)"),
        "missing from_ipc"
    );
    assert!(code.contains("text: str"), "missing text field");
}

#[test]
fn embedding_structure() {
    let code = compile_and_generate(&load_fixture("embedding.laic"));
    assert_common_structure(&code, "embedding");

    assert!(
        code.contains("class EmbeddingInput:"),
        "missing EmbeddingInput"
    );
    assert!(
        code.contains("class EmbeddingOutput:"),
        "missing EmbeddingOutput"
    );
    assert!(
        code.contains("SKILL_ID: ClassVar[str] = \"text-embedding\""),
        "missing SKILL_ID"
    );
    assert!(code.contains("text: str"), "missing text field");
    assert!(
        code.contains("model: str = \"default\""),
        "missing model default"
    );
    assert!(
        code.contains("max_tokens: int = 512"),
        "missing max_tokens default"
    );
    assert!(
        code.contains("embedding: bytes"),
        "missing embedding (tensor as bytes)"
    );
    assert!(code.contains("token_count: int"), "missing token_count");
    // Error enum
    assert!(
        code.contains("class EmbeddingError(enum.IntEnum):"),
        "missing EmbeddingError"
    );
    assert!(
        code.contains("INPUT_TOO_LONG = 1"),
        "missing INPUT_TOO_LONG"
    );
    assert!(
        code.contains("MODEL_NOT_FOUND = 2"),
        "missing MODEL_NOT_FOUND"
    );
}

#[test]
fn list_types_structure() {
    let code = compile_and_generate(&load_fixture("list_types.laic"));
    assert_common_structure(&code, "list_types");

    assert!(
        code.contains("class BatchEmbedInput:"),
        "missing BatchEmbedInput"
    );
    assert!(code.contains("texts: list[str]"), "missing texts field");
    assert!(
        code.contains("weights: list[float]"),
        "missing weights field"
    );
    assert!(
        code.contains("token_counts: list[int]"),
        "missing token_counts field"
    );
}

#[test]
fn map_types_structure() {
    let code = compile_and_generate(&load_fixture("map_types.laic"));
    assert_common_structure(&code, "map_types");

    assert!(code.contains("class MapDemoInput:"), "missing MapDemoInput");
    assert!(
        code.contains("metadata: dict[str, str]"),
        "missing metadata field"
    );
    assert!(
        code.contains("scores: dict[str, float]"),
        "missing scores field"
    );
    assert!(
        code.contains("flags: dict[int, bool]"),
        "missing flags field"
    );
    assert!(
        code.contains("results: dict[str, int]"),
        "missing results field"
    );
}

#[test]
fn optional_types_structure() {
    let code = compile_and_generate(&load_fixture("optional_types.laic"));
    assert_common_structure(&code, "optional_types");

    assert!(code.contains("class SearchInput:"), "missing SearchInput");
    assert!(
        code.contains("max_results: int | None"),
        "missing max_results"
    );
    assert!(
        code.contains("filter_tag: str | None"),
        "missing filter_tag"
    );
    assert!(
        code.contains("next_cursor: str | None"),
        "missing next_cursor"
    );
}

#[test]
fn tensor_container_structure() {
    let code = compile_and_generate(&load_fixture("tensor_container.laic"));
    assert_common_structure(&code, "tensor_container");

    assert!(
        code.contains("class TensorContainerInput:"),
        "missing TensorContainerInput"
    );
    assert!(
        code.contains("embeddings: list[bytes]"),
        "missing embeddings"
    );
    assert!(code.contains("primary: bytes"), "missing primary");
    assert!(code.contains("features: bytes | None"), "missing features");
}

#[test]
fn image_classify_structure() {
    let code = compile_and_generate(&load_fixture("image_classify.laic"));
    assert_common_structure(&code, "image_classify");

    assert!(
        code.contains("class ImageClassifyInput:"),
        "missing ImageClassifyInput"
    );
    assert!(code.contains("image_data: bytes"), "missing image_data");
    assert!(code.contains("top_k: int = 5"), "missing top_k default");
    assert!(
        code.contains("threshold: float = 0.5"),
        "missing threshold default"
    );
    assert!(
        code.contains("class ImageClassifyError(enum.IntEnum):"),
        "missing ImageClassifyError"
    );
}

#[test]
fn errors_only_structure() {
    let code = compile_and_generate(&load_fixture("errors_only.laic"));
    assert_common_structure(&code, "errors_only");

    assert!(
        code.contains("class HealthCheckError(enum.IntEnum):"),
        "missing HealthCheckError"
    );
    assert!(
        code.contains("SERVICE_UNAVAILABLE = 1"),
        "missing SERVICE_UNAVAILABLE"
    );
    assert!(code.contains("TIMEOUT = 2"), "missing TIMEOUT");
    assert!(
        code.contains("DEPENDENCY_FAILED = 4"),
        "missing DEPENDENCY_FAILED"
    );
}

#[test]
fn multi_skill_structure() {
    let code = compile_and_generate(&load_fixture("multi_skill.laic"));
    assert_common_structure(&code, "multi_skill");

    assert!(
        code.contains("class TokenizeInput:"),
        "missing TokenizeInput"
    );
    assert!(
        code.contains("class DetokenizeInput:"),
        "missing DetokenizeInput"
    );
    assert!(
        code.contains("SKILL_ID: ClassVar[str] = \"tokenize\""),
        "missing tokenize SKILL_ID"
    );
    assert!(
        code.contains("SKILL_ID: ClassVar[str] = \"detokenize\""),
        "missing detokenize SKILL_ID"
    );
    assert!(
        code.contains("VERSION: ClassVar[str] = \"2.0.0\""),
        "missing version 2.0.0"
    );
}

#[test]
fn string_defaults_are_escaped_for_python_source() {
    let code = compile_and_generate(&load_fixture("escaped_defaults.laic"));
    assert_common_structure(&code, "escaped_defaults");

    assert!(code.contains("message: str = \"line1\\nline2\""));
    assert!(!code.contains("message: str = \"line1\\r\\nline2\""));
    assert!(code.contains("path: str = \"C:\\\\temp\\\\file.txt\""));
    assert!(code.contains("note: str = \"line1\\nline2\""));
    assert!(!code.contains("note: str = \"line1\\r\\nline2\""));
    assert!(!code.contains("\"line1\nline2\""));
}

#[test]
fn nul_followed_by_digit_is_emitted_unambiguously_for_python_source() {
    let code = compile_and_generate(&nul_digit_defaults_source());
    assert_common_structure(&code, "nul_defaults");

    assert!(code.contains("payload: str = \"nul\\x001tail\""));
    assert!(!code.contains("payload: str = \"nul\\01tail\""));
}
