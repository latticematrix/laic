//! Integration tests for the laicc compiler pipeline.

use std::fs;

/// Helper: compile a fixture file end-to-end.
fn compile_fixture(name: &str) -> laicc::LaicFile {
    let path = format!("tests/fixtures/{name}.laic");
    let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    laicc::compile(&src).unwrap_or_else(|e| panic!("compile {path}: {e}"))
}

/// Helper: compile + generate Rust code for a fixture.
fn codegen_fixture(name: &str) -> String {
    let file = compile_fixture(name);
    laicc::generate_rust(&file).unwrap_or_else(|e| panic!("codegen {name}: {e}"))
}

#[test]
fn crate_root_support_type_reexports_remain_available() {
    use laicc::{compile, generate_rust, CompileError, LaicFile, LaicType, SkillDef};

    let src = r#"
        version "1.0.0";
        skill echo {
            id = "echo";
            input EchoInput { text: string; }
            output EchoOutput { text: string; }
        }
    "#;

    let file: LaicFile = compile(src).unwrap_or_else(|e| panic!("compile failed: {e}"));
    let skill: &SkillDef = &file.skills[0];
    assert!(matches!(skill.input.fields[0].ty, LaicType::String));

    let generated = generate_rust(&file).unwrap_or_else(|e| panic!("codegen failed: {e}"));
    assert!(generated.contains("pub struct EchoInput"));

    let err: CompileError = compile("not valid laic").expect_err("invalid source should fail");
    assert!(matches!(
        err,
        CompileError::Parse(_) | CompileError::Validation(_) | CompileError::Codegen(_)
    ));
}

// -----------------------------------------------------------------------
// Parse + validate fixtures
// -----------------------------------------------------------------------

#[test]
fn fixture_echo() {
    let file = compile_fixture("echo");
    assert_eq!(file.version, "1.0.0");
    assert_eq!(file.skills.len(), 1);
    assert_eq!(file.skills[0].name, "echo");
    assert_eq!(file.skills[0].id, "echo");
    assert_eq!(file.skills[0].input.fields.len(), 1);
    assert_eq!(file.skills[0].output.fields.len(), 1);
    assert!(file.skills[0].errors.is_empty());
}

#[test]
fn fixture_embedding() {
    let file = compile_fixture("embedding");
    assert_eq!(file.skills[0].name, "embedding");
    assert_eq!(file.skills[0].id, "text-embedding");
    assert_eq!(file.skills[0].input.fields.len(), 3);
    assert_eq!(file.skills[0].errors.len(), 3);

    // Check defaults
    let model_field = &file.skills[0].input.fields[1];
    assert_eq!(model_field.name, "model");
    assert_eq!(
        model_field.default,
        Some(laicc::Literal::String("default".into()))
    );

    let max_tokens = &file.skills[0].input.fields[2];
    assert_eq!(max_tokens.default, Some(laicc::Literal::Integer(512)));

    // Check tensor output
    let emb_field = &file.skills[0].output.fields[0];
    match &emb_field.ty {
        laicc::LaicType::Tensor { dtype, dims } => {
            assert_eq!(*dtype, laicc::TensorElementType::F32);
            assert_eq!(dims.len(), 1);
            assert_eq!(dims[0], laicc::Dimension::Fixed(768));
        }
        other => panic!("expected Tensor, got {other:?}"),
    }
}

#[test]
fn fixture_errors_only() {
    let file = compile_fixture("errors_only");
    assert_eq!(file.skills[0].errors.len(), 4);
    assert_eq!(file.skills[0].errors[0].name, "SERVICE_UNAVAILABLE");
    assert_eq!(file.skills[0].errors[0].code, 1);
    assert_eq!(file.skills[0].errors[3].name, "DEPENDENCY_FAILED");
    assert_eq!(file.skills[0].errors[3].code, 4);
}

#[test]
fn fixture_image_classify() {
    let file = compile_fixture("image_classify");
    let skill = &file.skills[0];
    assert_eq!(skill.id, "image-classify");

    // bytes field
    assert_eq!(skill.input.fields[0].ty, laicc::LaicType::Bytes);

    // defaults
    assert_eq!(
        skill.input.fields[1].default,
        Some(laicc::Literal::Integer(5))
    );
    assert_eq!(
        skill.input.fields[2].default,
        Some(laicc::Literal::Float(0.5))
    );

    // tensor with dynamic dim
    match &skill.output.fields[0].ty {
        laicc::LaicType::Tensor { dims, .. } => {
            assert_eq!(dims.len(), 2);
            assert_eq!(dims[0], laicc::Dimension::Dynamic(None));
            assert_eq!(dims[1], laicc::Dimension::Fixed(512));
        }
        other => panic!("expected Tensor, got {other:?}"),
    }
}

#[test]
fn fixture_multi_skill() {
    let file = compile_fixture("multi_skill");
    assert_eq!(file.version, "2.0.0");
    assert_eq!(file.skills.len(), 2);
    assert_eq!(file.skills[0].name, "tokenize");
    assert_eq!(file.skills[1].name, "detokenize");
}

#[test]
fn fixture_list_types() {
    let file = compile_fixture("list_types");
    let skill = &file.skills[0];

    match &skill.input.fields[0].ty {
        laicc::LaicType::List(inner) => {
            assert_eq!(**inner, laicc::LaicType::String);
        }
        other => panic!("expected List, got {other:?}"),
    }

    match &skill.input.fields[1].ty {
        laicc::LaicType::List(inner) => {
            assert_eq!(**inner, laicc::LaicType::F32);
        }
        other => panic!("expected List, got {other:?}"),
    }
}

#[test]
fn fixture_optional_types() {
    let file = compile_fixture("optional_types");
    let skill = &file.skills[0];

    match &skill.input.fields[1].ty {
        laicc::LaicType::Optional(inner) => {
            assert_eq!(**inner, laicc::LaicType::I32);
        }
        other => panic!("expected Optional, got {other:?}"),
    }

    match &skill.output.fields[2].ty {
        laicc::LaicType::Optional(inner) => {
            assert_eq!(**inner, laicc::LaicType::String);
        }
        other => panic!("expected Optional, got {other:?}"),
    }
}

#[test]
fn fixture_map_types() {
    let file = compile_fixture("map_types");
    let skill = &file.skills[0];

    match &skill.input.fields[0].ty {
        laicc::LaicType::Map(k, v) => {
            assert_eq!(**k, laicc::LaicType::String);
            assert_eq!(**v, laicc::LaicType::String);
        }
        other => panic!("expected Map, got {other:?}"),
    }
}

#[test]
fn fixture_tensor_container() {
    let file = compile_fixture("tensor_container");
    let skill = &file.skills[0];

    // list<tensor<f32>[768]>
    match &skill.input.fields[0].ty {
        laicc::LaicType::List(inner) => match inner.as_ref() {
            laicc::LaicType::Tensor { dtype, dims } => {
                assert_eq!(*dtype, laicc::TensorElementType::F32);
                assert_eq!(dims.len(), 1);
                assert_eq!(dims[0], laicc::Dimension::Fixed(768));
            }
            other => panic!("expected Tensor inside List, got {other:?}"),
        },
        other => panic!("expected List, got {other:?}"),
    }

    // optional<tensor<f64>[512]>
    match &skill.output.fields[0].ty {
        laicc::LaicType::Optional(inner) => match inner.as_ref() {
            laicc::LaicType::Tensor { dtype, dims } => {
                assert_eq!(*dtype, laicc::TensorElementType::F64);
                assert_eq!(dims.len(), 1);
                assert_eq!(dims[0], laicc::Dimension::Fixed(512));
            }
            other => panic!("expected Tensor inside Optional, got {other:?}"),
        },
        other => panic!("expected Optional, got {other:?}"),
    }
}

// -----------------------------------------------------------------------
// Codegen tests — verify generated Rust code structure
// -----------------------------------------------------------------------

#[test]
fn codegen_echo_contains_struct_and_methods() {
    let code = codegen_fixture("echo");
    assert!(code.contains("pub struct EchoInput"));
    assert!(code.contains("pub struct EchoOutput"));
    assert!(code.contains("pub fn to_arrow_ipc"));
    assert!(code.contains("pub fn from_arrow_ipc"));
    assert!(code.contains("laic.skill_id"));
    assert!(code.contains("\"echo\""));
}

#[test]
fn codegen_embedding_contains_error_enum() {
    let code = codegen_fixture("embedding");
    assert!(code.contains("pub enum EmbeddingError"));
    assert!(code.contains("InputTooLong = 1"));
    assert!(code.contains("ModelNotFound = 2"));
    assert!(code.contains("InferenceFailed = 3"));
    assert!(code.contains("pub fn from_code"));
    assert!(code.contains("pub fn code(&self)"));
    assert!(code.contains("pub fn name(&self)"));
}

#[test]
fn codegen_multi_skill_generates_all() {
    let code = codegen_fixture("multi_skill");
    assert!(code.contains("pub struct TokenizeInput"));
    assert!(code.contains("pub struct TokenizeOutput"));
    assert!(code.contains("pub struct DetokenizeInput"));
    assert!(code.contains("pub struct DetokenizeOutput"));
}

#[test]
fn codegen_header_contains_dependency_note() {
    let code = codegen_fixture("echo");
    assert!(code.contains("Generated by laicc"));
    assert!(code.contains("DO NOT EDIT"));
    assert!(code.contains("arrow-array"));
}

#[test]
fn codegen_map_types_uses_hashmap() {
    let code = codegen_fixture("map_types");
    assert!(code.contains("HashMap<String, String>"));
    assert!(code.contains("HashMap<String, f64>"));
    assert!(code.contains("HashMap<i32, bool>"));
}

#[test]
fn codegen_optional_types_uses_option() {
    let code = codegen_fixture("optional_types");
    assert!(code.contains("Option<i32>"));
    assert!(code.contains("Option<String>"));
    assert!(code.contains("Option<f64>"));
}

#[test]
fn codegen_escaped_defaults_use_logical_newlines() {
    let code = codegen_fixture("escaped_defaults");
    assert!(code.contains("\"line1\\nline2\".to_string()"));
    assert!(!code.contains("\"line1\\r\\nline2\".to_string()"));
}

#[test]
fn codegen_metadata_literals_escape_backslash_and_newline() {
    let src = "version \"1.0\\dev\nnext\";\n\
        skill metadata_escape {\n\
            id = \"bad\\id\nnext\";\n\
            input MetadataEscapeInput { text: string; }\n\
            output MetadataEscapeOutput { text: string; }\n\
        }\n";
    let file = laicc::compile(src).unwrap_or_else(|e| panic!("compile failed: {e}"));

    let rust = laicc::generate_rust(&file).unwrap_or_else(|e| panic!("rust codegen failed: {e}"));
    assert!(rust.contains("pub fn skill_id() -> &'static str { \"bad\\\\id\\nnext\" }"));
    assert!(rust.contains("pub fn version() -> &'static str { \"1.0\\\\dev\\nnext\" }"));
    assert!(rust.contains("(\"laic.skill_id\".into(), \"bad\\\\id\\nnext\".into())"));
    assert!(rust.contains("Some(v) if v != \"1.0\\\\dev\\nnext\""));

    let python =
        laicc::generate_python(&file).unwrap_or_else(|e| panic!("python codegen failed: {e}"));
    assert!(python.contains("SKILL_ID: ClassVar[str] = \"bad\\\\id\\nnext\""));
    assert!(python.contains("VERSION: ClassVar[str] = \"1.0\\\\dev\\nnext\""));
    assert!(python.contains("b\"laic.skill_id\": \"bad\\\\id\\nnext\".encode(\"utf-8\")"));
    assert!(python.contains("if _v != \"1.0\\\\dev\\nnext\".encode(\"utf-8\")"));

    let ts = laicc::generate_typescript(&file).unwrap_or_else(|e| panic!("ts codegen failed: {e}"));
    assert!(ts.contains("static readonly SKILL_ID = \"bad\\\\id\\nnext\";"));
    assert!(ts.contains("static readonly VERSION = \"1.0\\\\dev\\nnext\";"));
    assert!(ts.contains("[\"laic.skill_id\", \"bad\\\\id\\nnext\"]"));
    assert!(
        ts.contains("laicAssertMetadata(schemaMetadata, \"laic.version\", \"1.0\\\\dev\\nnext\")")
    );
}

// -----------------------------------------------------------------------
// Negative tests — invalid .laic sources
// -----------------------------------------------------------------------

#[test]
fn reject_empty_source() {
    let result = laicc::compile("");
    assert!(result.is_err());
}

#[test]
fn reject_no_skills() {
    let src = r#"version "1.0.0";"#;
    let result = laicc::compile(src);
    assert!(result.is_err());
}

#[test]
fn reject_duplicate_skill_names() {
    let src = r#"
        version "1.0.0";
        skill foo { id = "a"; input A { x: i32; } output B { y: i32; } }
        skill foo { id = "b"; input C { x: i32; } output D { y: i32; } }
    "#;
    let result = laicc::compile(src);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("duplicate skill name"));
}

#[test]
fn reject_empty_input_struct() {
    let src = r#"
        version "1.0.0";
        skill test { id = "t"; input A {} output B { y: i32; } }
    "#;
    let result = laicc::compile(src);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("at least one field"));
}
