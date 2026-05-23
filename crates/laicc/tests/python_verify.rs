//! Tier 2 (Python syntax) + Tier 3 (roundtrip) verification.
//!
//! Feature-gated: `cargo test --features python-verify`
//! Requires: Python 3.x + pyarrow installed.

#![cfg(feature = "python-verify")]

mod python_verify_support;

use std::process::Command;

use python_verify_support::{
    assert_process_succeeded, driver_path, fresh_case_dir, python_command, python_driver_script,
    write_generated_package, EMBEDDING_INPUT_MODEL_TYPE_MISMATCH_SCRIPT,
    LIST_TENSOR_METADATA_REQUIRED_SCRIPT, OPTIONAL_TENSOR_METADATA_REQUIRED_SCRIPT,
};

fn compile_and_generate(source: &str) -> String {
    let file = laicc::compile(source).unwrap_or_else(|e| panic!("compile failed: {e}"));
    laicc::generate_python(&file).unwrap_or_else(|e| panic!("generate_python failed: {e}"))
}

fn load_fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read fixture {name}: {e}"))
}

/// Tier 2: verify generated code is valid Python syntax.
fn verify_syntax(code: &str, fixture: &str) {
    let case_dir = fresh_case_dir(&format!("syntax_{fixture}"));
    let module_name = write_generated_package(&case_dir, fixture, code);
    let path = case_dir.join("generated").join(format!("{module_name}.py"));
    let output = Command::new("python")
        .args([
            "-c",
            &format!(
                "import ast; ast.parse(open(r'{}', encoding='utf-8').read()); print('SYNTAX_OK')",
                path.display()
            ),
        ])
        .output()
        .unwrap_or_else(|e| panic!("python not found: {e}"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("SYNTAX_OK"),
        "Tier 2 syntax check failed for {fixture}:\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// Tier 3: write generated code as a temp package, then execute a separate driver via import.
fn verify_roundtrip(code: &str, test_body: &str, fixture: &str) {
    let case_dir = fresh_case_dir(&format!("roundtrip_{fixture}"));
    write_generated_package(&case_dir, fixture, code);
    let driver_path = driver_path(&case_dir);
    let driver = python_driver_script(fixture, test_body);
    std::fs::write(&driver_path, driver).unwrap_or_else(|e| panic!("write driver {fixture}: {e}"));

    let output = python_command(&case_dir)
        .arg(driver_path.to_str().unwrap_or(""))
        .output()
        .unwrap_or_else(|e| panic!("python not found: {e}"));

    assert_process_succeeded(&output, fixture);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("PASS"),
        "Tier 3 roundtrip failed for {fixture}: stdout did not contain PASS\nstdout: {stdout}"
    );
}

// -- Echo --

#[test]
fn echo_syntax() {
    let code = compile_and_generate(&load_fixture("echo.laic"));
    verify_syntax(&code, "echo");
}

#[test]
fn echo_roundtrip() {
    let code = compile_and_generate(&load_fixture("echo.laic"));
    verify_roundtrip(
        &code,
        r#"
inp = EchoInput(text="hello world")
data = inp.to_ipc()
restored = EchoInput.from_ipc(data)
assert restored.text == "hello world", f"got {restored.text}"

out = EchoOutput(text="echo: hello")
data = out.to_ipc()
restored = EchoOutput.from_ipc(data)
assert restored.text == "echo: hello"

print("PASS")
"#,
        "echo",
    );
}

#[test]
fn multiline_string_defaults_syntax() {
    let code = compile_and_generate(&load_fixture("escaped_defaults.laic"));
    verify_syntax(&code, "escaped_defaults");
}

// -- Embedding (scalars + tensor + defaults + error enum) --

#[test]
fn embedding_syntax() {
    let code = compile_and_generate(&load_fixture("embedding.laic"));
    verify_syntax(&code, "embedding");
}

#[test]
fn embedding_roundtrip() {
    let code = compile_and_generate(&load_fixture("embedding.laic"));
    verify_roundtrip(
        &code,
        r#"
# Test with explicit values
inp = EmbeddingInput(text="test", model="ada", max_tokens=256)
data = inp.to_ipc()
r = EmbeddingInput.from_ipc(data)
assert r.text == "test"
assert r.model == "ada"
assert r.max_tokens == 256

# Test with defaults
inp2 = EmbeddingInput(text="hello")
data2 = inp2.to_ipc()
r2 = EmbeddingInput.from_ipc(data2)
assert r2.model == "default"
assert r2.max_tokens == 512

# Test output with tensor
embedding_bytes = b"\x00" * (768 * 4)  # 768 f32 values
out = EmbeddingOutput(embedding=embedding_bytes, token_count=42)
data = out.to_ipc()
r = EmbeddingOutput.from_ipc(data)
assert r.embedding == embedding_bytes
assert r.token_count == 42

# Test error enum
assert EmbeddingError.INPUT_TOO_LONG == 1
assert EmbeddingError.MODEL_NOT_FOUND == 2
assert EmbeddingError.INFERENCE_FAILED == 3

print("PASS")
"#,
        "embedding",
    );
}

#[test]
fn embedding_default_field_type_mismatch_rejected() {
    let code = compile_and_generate(&load_fixture("embedding.laic"));
    verify_roundtrip(
        &code,
        EMBEDDING_INPUT_MODEL_TYPE_MISMATCH_SCRIPT,
        "embedding_default_field_type_mismatch_rejected",
    );
}

// -- List types --

#[test]
fn list_types_syntax() {
    let code = compile_and_generate(&load_fixture("list_types.laic"));
    verify_syntax(&code, "list_types");
}

#[test]
fn list_types_roundtrip() {
    let code = compile_and_generate(&load_fixture("list_types.laic"));
    verify_roundtrip(
        &code,
        r#"
inp = BatchEmbedInput(texts=["hello", "world"], weights=[0.5, 1.0], model="test")
data = inp.to_ipc()
r = BatchEmbedInput.from_ipc(data)
assert r.texts == ["hello", "world"], f"got {r.texts}"
assert abs(r.weights[0] - 0.5) < 1e-6
assert abs(r.weights[1] - 1.0) < 1e-6
assert r.model == "test"

out = BatchEmbedOutput(token_counts=[10, 20], success=True)
data = out.to_ipc()
r = BatchEmbedOutput.from_ipc(data)
assert r.token_counts == [10, 20]
assert r.success is True

print("PASS")
"#,
        "list_types",
    );
}

// -- Map types --

#[test]
fn map_types_syntax() {
    let code = compile_and_generate(&load_fixture("map_types.laic"));
    verify_syntax(&code, "map_types");
}

#[test]
fn map_types_roundtrip() {
    let code = compile_and_generate(&load_fixture("map_types.laic"));
    verify_roundtrip(
        &code,
        r#"
inp = MapDemoInput(
    metadata={"key1": "val1", "key2": "val2"},
    scores={"a": 1.5, "b": 2.5},
    flags={1: True, 2: False},
)
data = inp.to_ipc()
r = MapDemoInput.from_ipc(data)
assert r.metadata == {"key1": "val1", "key2": "val2"}, f"got {r.metadata}"
assert abs(r.scores["a"] - 1.5) < 1e-6
assert r.flags[1] is True
assert r.flags[2] is False

out = MapDemoOutput(results={"x": 10}, labels={"y": "z"})
data = out.to_ipc()
r = MapDemoOutput.from_ipc(data)
assert r.results == {"x": 10}
assert r.labels == {"y": "z"}

print("PASS")
"#,
        "map_types",
    );
}

// -- Optional types --

#[test]
fn optional_types_syntax() {
    let code = compile_and_generate(&load_fixture("optional_types.laic"));
    verify_syntax(&code, "optional_types");
}

#[test]
fn optional_types_roundtrip() {
    let code = compile_and_generate(&load_fixture("optional_types.laic"));
    verify_roundtrip(
        &code,
        r#"
# With values
inp = SearchInput(query="test", max_results=10, filter_tag="news", threshold=0.8)
data = inp.to_ipc()
r = SearchInput.from_ipc(data)
assert r.query == "test"
assert r.max_results == 10
assert r.filter_tag == "news"
assert abs(r.threshold - 0.8) < 1e-6

# With None
inp2 = SearchInput(query="test2", max_results=None, filter_tag=None, threshold=None)
data2 = inp2.to_ipc()
r2 = SearchInput.from_ipc(data2)
assert r2.query == "test2"
assert r2.max_results is None
assert r2.filter_tag is None
assert r2.threshold is None

out = SearchOutput(results=["a", "b"], total_count=2, next_cursor="abc")
data = out.to_ipc()
r = SearchOutput.from_ipc(data)
assert r.results == ["a", "b"]
assert r.total_count == 2
assert r.next_cursor == "abc"

# next_cursor = None
out2 = SearchOutput(results=[], total_count=0, next_cursor=None)
data2 = out2.to_ipc()
r2 = SearchOutput.from_ipc(data2)
assert r2.next_cursor is None

print("PASS")
"#,
        "optional_types",
    );
}

// -- Tensor container --

#[test]
fn tensor_container_syntax() {
    let code = compile_and_generate(&load_fixture("tensor_container.laic"));
    verify_syntax(&code, "tensor_container");
}

#[test]
fn tensor_container_roundtrip() {
    let code = compile_and_generate(&load_fixture("tensor_container.laic"));
    verify_roundtrip(
        &code,
        r#"
emb1 = b"\x01" * 100
emb2 = b"\x02" * 200
primary = b"\x03" * (3 * 224 * 224 * 4)

inp = TensorContainerInput(embeddings=[emb1, emb2], primary=primary)
data = inp.to_ipc()
r = TensorContainerInput.from_ipc(data)
assert r.embeddings == [emb1, emb2], "embeddings mismatch"
assert r.primary == primary, "primary mismatch"

# Output with optional tensor present
scores_t = b"\x04" * 40
out = TensorContainerOutput(features=b"\x05" * 100, scores=[scores_t])
data = out.to_ipc()
r = TensorContainerOutput.from_ipc(data)
assert r.features == b"\x05" * 100
assert r.scores == [scores_t]

# Output with optional tensor None
out2 = TensorContainerOutput(features=None, scores=[])
data2 = out2.to_ipc()
r2 = TensorContainerOutput.from_ipc(data2)
assert r2.features is None

print("PASS")
"#,
        "tensor_container",
    );
}

#[test]
fn list_tensor_metadata_required() {
    let code = compile_and_generate(&load_fixture("tensor_container.laic"));
    verify_roundtrip(
        &code,
        LIST_TENSOR_METADATA_REQUIRED_SCRIPT,
        "list_tensor_metadata_required",
    );
}

#[test]
fn optional_tensor_metadata_required_even_when_null() {
    let code = compile_and_generate(&load_fixture("tensor_container.laic"));
    verify_roundtrip(
        &code,
        OPTIONAL_TENSOR_METADATA_REQUIRED_SCRIPT,
        "optional_tensor_metadata_required_even_when_null",
    );
}

// -- Image classify --

#[test]
fn image_classify_syntax() {
    let code = compile_and_generate(&load_fixture("image_classify.laic"));
    verify_syntax(&code, "image_classify");
}

#[test]
fn image_classify_roundtrip() {
    let code = compile_and_generate(&load_fixture("image_classify.laic"));
    verify_roundtrip(
        &code,
        r#"
inp = ImageClassifyInput(image_data=b"\xff\xd8\xff", top_k=3, threshold=0.7)
data = inp.to_ipc()
r = ImageClassifyInput.from_ipc(data)
assert r.image_data == b"\xff\xd8\xff"
assert r.top_k == 3
assert abs(r.threshold - 0.7) < 1e-6

features = b"\x00" * 512
out = ImageClassifyOutput(features=features, label="cat", confidence=0.95)
data = out.to_ipc()
r = ImageClassifyOutput.from_ipc(data)
assert r.features == features
assert r.label == "cat"
assert abs(r.confidence - 0.95) < 1e-6

print("PASS")
"#,
        "image_classify",
    );
}

// -- Errors only --

#[test]
fn errors_only_syntax() {
    let code = compile_and_generate(&load_fixture("errors_only.laic"));
    verify_syntax(&code, "errors_only");
}

#[test]
fn errors_only_roundtrip() {
    let code = compile_and_generate(&load_fixture("errors_only.laic"));
    verify_roundtrip(
        &code,
        r#"
inp = HealthInput(service_name="db")
data = inp.to_ipc()
r = HealthInput.from_ipc(data)
assert r.service_name == "db"

out = HealthOutput(status=200, message="ok")
data = out.to_ipc()
r = HealthOutput.from_ipc(data)
assert r.status == 200
assert r.message == "ok"

assert HealthCheckError.SERVICE_UNAVAILABLE == 1
assert HealthCheckError.TIMEOUT == 2
assert HealthCheckError.DEPENDENCY_FAILED == 4

print("PASS")
"#,
        "errors_only",
    );
}

// -- Multi skill --

#[test]
fn multi_skill_syntax() {
    let code = compile_and_generate(&load_fixture("multi_skill.laic"));
    verify_syntax(&code, "multi_skill");
}

#[test]
fn multi_skill_roundtrip() {
    let code = compile_and_generate(&load_fixture("multi_skill.laic"));
    verify_roundtrip(
        &code,
        r#"
# Tokenize
inp = TokenizeInput(text="hello", max_length=128)
data = inp.to_ipc()
r = TokenizeInput.from_ipc(data)
assert r.text == "hello"
assert r.max_length == 128

token_data = b"\x01\x02\x03\x04"
mask_data = b"\x01\x01\x01\x00"
out = TokenizeOutput(token_ids=token_data, attention_mask=mask_data)
data = out.to_ipc()
r = TokenizeOutput.from_ipc(data)
assert r.token_ids == token_data
assert r.attention_mask == mask_data

# Detokenize
inp2 = DetokenizeInput(token_ids=token_data)
data2 = inp2.to_ipc()
r2 = DetokenizeInput.from_ipc(data2)
assert r2.token_ids == token_data

out2 = DetokenizeOutput(text="hello")
data2 = out2.to_ipc()
r2 = DetokenizeOutput.from_ipc(data2)
assert r2.text == "hello"

print("PASS")
"#,
        "multi_skill",
    );
}
