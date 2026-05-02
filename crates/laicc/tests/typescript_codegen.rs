//! Tier 1: structural validation of TypeScript codegen output (zero dependencies).
//!
//! These tests should run as part of regular `cargo test` without requiring npm or Node.

mod support;

use support::nul_digit_defaults_source;

fn compile_and_generate(source: &str) -> String {
    let file = laicc::compile(source).unwrap_or_else(|e| panic!("compile failed: {e}"));
    laicc::generate_typescript(&file).unwrap_or_else(|e| panic!("generate_typescript failed: {e}"))
}

fn load_fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read fixture {name}: {e}"))
}

fn assert_common_structure(code: &str, fixture: &str) {
    assert!(
        code.contains("import * as arrow from \"apache-arrow\""),
        "{fixture}: missing apache-arrow import"
    );
    assert!(
        code.contains("toIpc(): Uint8Array"),
        "{fixture}: missing toIpc"
    );
    assert!(
        code.contains("static fromIpc(data: Uint8Array)"),
        "{fixture}: missing fromIpc"
    );
    assert!(
        !code.contains("export interface EchoInput"),
        "{fixture}: unexpected interface-based contract output"
    );
}

#[test]
fn echo_structure() {
    let code = compile_and_generate(&load_fixture("echo.laic"));
    assert_common_structure(&code, "echo");

    assert!(code.contains("export class EchoInput"));
    assert!(code.contains("export class EchoOutput"));
    assert!(code.contains("static readonly SKILL_ID = \"echo\""));
    assert!(code.contains("static readonly VERSION = \"1.0.0\""));
    assert!(code.contains("static readonly DIRECTION = \"input\""));
    assert!(code.contains("static readonly DIRECTION = \"output\""));
    assert!(code.contains("public readonly text: string"));
}

#[test]
fn embedding_structure() {
    let code = compile_and_generate(&load_fixture("embedding.laic"));
    assert_common_structure(&code, "embedding");

    assert!(code.contains("export class EmbeddingInput"));
    assert!(code.contains("export class EmbeddingOutput"));
    assert!(code.contains("static readonly SKILL_ID = \"text-embedding\""));
    assert!(code.contains("public readonly text: string"));
    assert!(code.contains("public readonly model: string = \"default\""));
    assert!(code.contains("public readonly max_tokens: number = 512"));
    assert!(code.contains("public readonly embedding: Uint8Array"));
    assert!(code.contains("public readonly token_count: number"));
    assert!(code.contains("export enum EmbeddingError"));
    assert!(code.contains("INPUT_TOO_LONG = 1"));
    assert!(code.contains("MODEL_NOT_FOUND = 2"));
}

#[test]
fn list_types_structure() {
    let code = compile_and_generate(&load_fixture("list_types.laic"));
    assert_common_structure(&code, "list_types");

    assert!(code.contains("export class BatchEmbedInput"));
    assert!(code.contains("public readonly texts: string[]"));
    assert!(code.contains("public readonly weights: number[]"));
    assert!(code.contains("public readonly token_counts: number[]"));
}

#[test]
fn map_types_structure() {
    let code = compile_and_generate(&load_fixture("map_types.laic"));
    assert_common_structure(&code, "map_types");

    assert!(code.contains("export class MapDemoInput"));
    assert!(code.contains("public readonly metadata: Map<string, string>"));
    assert!(code.contains("public readonly scores: Map<string, number>"));
    assert!(code.contains("public readonly flags: Map<number, boolean>"));
    assert!(code.contains("public readonly results: Map<string, number>"));
}

#[test]
fn optional_types_structure() {
    let code = compile_and_generate(&load_fixture("optional_types.laic"));
    assert_common_structure(&code, "optional_types");

    assert!(code.contains("export class SearchInput"));
    assert!(code.contains("public readonly max_results: number | null"));
    assert!(code.contains("public readonly filter_tag: string | null"));
    assert!(code.contains("public readonly next_cursor: string | null"));
}

#[test]
fn tensor_container_structure() {
    let code = compile_and_generate(&load_fixture("tensor_container.laic"));
    assert_common_structure(&code, "tensor_container");

    assert!(code.contains("export class TensorContainerInput"));
    assert!(code.contains("public readonly embeddings: Uint8Array[]"));
    assert!(code.contains("public readonly primary: Uint8Array"));
    assert!(code.contains("public readonly features: Uint8Array | null"));
    assert!(code.contains("public readonly scores: Uint8Array[]"));
}

#[test]
fn image_classify_structure() {
    let code = compile_and_generate(&load_fixture("image_classify.laic"));
    assert_common_structure(&code, "image_classify");

    assert!(code.contains("export class ImageClassifyInput"));
    assert!(code.contains("public readonly image_data: Uint8Array"));
    assert!(code.contains("public readonly top_k: number = 5"));
    assert!(code.contains("public readonly threshold: number = 0.5"));
    assert!(code.contains("export enum ImageClassifyError"));
}

#[test]
fn errors_only_structure() {
    let code = compile_and_generate(&load_fixture("errors_only.laic"));
    assert_common_structure(&code, "errors_only");

    assert!(code.contains("export enum HealthCheckError"));
    assert!(code.contains("SERVICE_UNAVAILABLE = 1"));
    assert!(code.contains("TIMEOUT = 2"));
    assert!(code.contains("DEPENDENCY_FAILED = 4"));
}

#[test]
fn multi_skill_structure() {
    let code = compile_and_generate(&load_fixture("multi_skill.laic"));
    assert_common_structure(&code, "multi_skill");

    assert!(code.contains("export class TokenizeInput"));
    assert!(code.contains("export class DetokenizeInput"));
    assert!(code.contains("static readonly SKILL_ID = \"tokenize\""));
    assert!(code.contains("static readonly SKILL_ID = \"detokenize\""));
    assert!(code.contains("static readonly VERSION = \"2.0.0\""));
}

#[test]
fn string_defaults_are_escaped_for_typescript_source() {
    let code = compile_and_generate(&load_fixture("escaped_defaults.laic"));
    assert_common_structure(&code, "escaped_defaults");

    assert!(code.contains("public readonly message: string = \"line1\\nline2\""));
    assert!(!code.contains("public readonly message: string = \"line1\\r\\nline2\""));
    assert!(code.contains("public readonly path: string = \"C:\\\\temp\\\\file.txt\""));
    assert!(code.contains("public readonly note: string = \"line1\\nline2\""));
    assert!(!code.contains("public readonly note: string = \"line1\\r\\nline2\""));
    assert!(!code.contains("\"line1\nline2\""));
}

#[test]
fn nul_followed_by_digit_is_emitted_unambiguously_for_typescript_source() {
    let code = compile_and_generate(&nul_digit_defaults_source());
    assert_common_structure(&code, "nul_defaults");

    assert!(code.contains("public readonly payload: string = \"nul\\x001tail\""));
    assert!(!code.contains("public readonly payload: string = \"nul\\01tail\""));
}
