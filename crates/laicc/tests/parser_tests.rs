//! Parser unit tests.

#![allow(clippy::unwrap_used)]

use laicc::{compile, Dimension, LaicType, Literal, TensorElementType};

#[test]
fn test_parse_minimal() {
    let src = r#"
        version "1.0.0";
        skill echo {
            id = "echo";
            input EchoInput { text: string; }
            output EchoOutput { text: string; }
        }
    "#;
    let file = compile(src).unwrap_or_else(|e| panic!("compile failed: {e}"));
    assert_eq!(file.version, "1.0.0");
    assert_eq!(file.skills.len(), 1);
    assert_eq!(file.skills[0].name, "echo");
    assert_eq!(file.skills[0].id, "echo");
    assert_eq!(file.skills[0].input.name, "EchoInput");
    assert_eq!(file.skills[0].input.fields.len(), 1);
    assert_eq!(file.skills[0].input.fields[0].name, "text");
    assert_eq!(file.skills[0].input.fields[0].ty, LaicType::String);
}

#[test]
fn test_parse_all_scalar_types() {
    let src = r#"
        version "1.0.0";
        skill test {
            id = "test";
            input TestInput {
                a: string;
                b: bytes;
                c: bool;
                d: i8;
                e: i16;
                f: i32;
                g: i64;
                h: u8;
                i: f32;
                j: f64;
            }
            output TestOutput { x: i32; }
        }
    "#;
    let file = compile(src).unwrap_or_else(|e| panic!("compile failed: {e}"));
    let fields = &file.skills[0].input.fields;
    assert_eq!(fields.len(), 10);
    assert_eq!(fields[0].ty, LaicType::String);
    assert_eq!(fields[1].ty, LaicType::Bytes);
    assert_eq!(fields[2].ty, LaicType::Bool);
    assert_eq!(fields[3].ty, LaicType::I8);
    assert_eq!(fields[4].ty, LaicType::I16);
    assert_eq!(fields[5].ty, LaicType::I32);
    assert_eq!(fields[6].ty, LaicType::I64);
    assert_eq!(fields[7].ty, LaicType::U8);
    assert_eq!(fields[8].ty, LaicType::F32);
    assert_eq!(fields[9].ty, LaicType::F64);
}

#[test]
fn test_parse_tensor() {
    let src = r#"
        version "1.0.0";
        skill test {
            id = "test";
            input TestInput { emb: tensor<f32>[768]; }
            output TestOutput { x: i32; }
        }
    "#;
    let file = compile(src).unwrap_or_else(|e| panic!("compile failed: {e}"));
    match &file.skills[0].input.fields[0].ty {
        LaicType::Tensor { dtype, dims } => {
            assert_eq!(*dtype, TensorElementType::F32);
            assert_eq!(dims.len(), 1);
            assert_eq!(dims[0], Dimension::Fixed(768));
        }
        other => panic!("expected Tensor, got {other:?}"),
    }
}

#[test]
fn test_parse_tensor_dynamic_dims() {
    let src = r#"
        version "1.0.0";
        skill test {
            id = "test";
            input TestInput { emb: tensor<f32>[_, 768]; }
            output TestOutput { x: i32; }
        }
    "#;
    let file = compile(src).unwrap_or_else(|e| panic!("compile failed: {e}"));
    match &file.skills[0].input.fields[0].ty {
        LaicType::Tensor { dims, .. } => {
            assert_eq!(dims.len(), 2);
            assert_eq!(dims[0], Dimension::Dynamic(None));
            assert_eq!(dims[1], Dimension::Fixed(768));
        }
        other => panic!("expected Tensor, got {other:?}"),
    }
}

#[test]
fn test_parse_tensor_multidim() {
    let src = r#"
        version "1.0.0";
        skill test {
            id = "test";
            input TestInput { img: tensor<f32>[3, 224, 224]; }
            output TestOutput { x: i32; }
        }
    "#;
    let file = compile(src).unwrap_or_else(|e| panic!("compile failed: {e}"));
    match &file.skills[0].input.fields[0].ty {
        LaicType::Tensor { dims, .. } => {
            assert_eq!(dims.len(), 3);
            assert_eq!(dims[0], Dimension::Fixed(3));
            assert_eq!(dims[1], Dimension::Fixed(224));
            assert_eq!(dims[2], Dimension::Fixed(224));
        }
        other => panic!("expected Tensor, got {other:?}"),
    }
}

#[test]
fn test_parse_defaults() {
    let src = r#"
        version "1.0.0";
        skill test {
            id = "test";
            input TestInput {
                model: string = "default";
                max_tokens: i32 = 512;
                threshold: f64 = 0.5;
                enabled: bool = true;
            }
            output TestOutput { x: i32; }
        }
    "#;
    let file = compile(src).unwrap_or_else(|e| panic!("compile failed: {e}"));
    let fields = &file.skills[0].input.fields;
    assert_eq!(fields[0].default, Some(Literal::String("default".into())));
    assert_eq!(fields[1].default, Some(Literal::Integer(512)));
    assert_eq!(fields[2].default, Some(Literal::Float(0.5)));
    assert_eq!(fields[3].default, Some(Literal::Bool(true)));
}

#[test]
fn test_parse_errors() {
    let src = r#"
        version "1.0.0";
        skill test {
            id = "test";
            input TestInput { x: i32; }
            output TestOutput { y: i32; }
            error {
                INPUT_TOO_LONG = 1;
                MODEL_NOT_FOUND = 2;
            }
        }
    "#;
    let file = compile(src).unwrap_or_else(|e| panic!("compile failed: {e}"));
    assert_eq!(file.skills[0].errors.len(), 2);
    assert_eq!(file.skills[0].errors[0].name, "INPUT_TOO_LONG");
    assert_eq!(file.skills[0].errors[0].code, 1);
    assert_eq!(file.skills[0].errors[1].name, "MODEL_NOT_FOUND");
    assert_eq!(file.skills[0].errors[1].code, 2);
}

#[test]
fn test_parse_multiple_skills() {
    let src = r#"
        version "1.0.0";
        skill a {
            id = "a";
            input AI { x: i32; }
            output AO { y: i32; }
        }
        skill b {
            id = "b";
            input BI { x: string; }
            output BO { y: string; }
        }
    "#;
    let file = compile(src).unwrap_or_else(|e| panic!("compile failed: {e}"));
    assert_eq!(file.skills.len(), 2);
    assert_eq!(file.skills[0].name, "a");
    assert_eq!(file.skills[1].name, "b");
}

#[test]
fn test_parse_comments() {
    let src = r#"
        // This is a comment
        version "1.0.0";
        // Another comment
        skill echo {
            id = "echo";
            input EchoInput {
                // field comment
                text: string;
            }
            output EchoOutput { text: string; }
        }
    "#;
    let file = compile(src).unwrap_or_else(|e| panic!("compile failed: {e}"));
    assert_eq!(file.skills.len(), 1);
}

#[test]
fn test_parse_list_optional_map() {
    let src = r#"
        version "1.0.0";
        skill test {
            id = "test";
            input TestInput {
                tags: list<string>;
                maybe: optional<i32>;
                meta: map<string, f64>;
            }
            output TestOutput { x: i32; }
        }
    "#;
    let file = compile(src).unwrap_or_else(|e| panic!("compile failed: {e}"));
    let fields = &file.skills[0].input.fields;

    match &fields[0].ty {
        LaicType::List(inner) => assert_eq!(**inner, LaicType::String),
        other => panic!("expected List, got {other:?}"),
    }
    match &fields[1].ty {
        LaicType::Optional(inner) => assert_eq!(**inner, LaicType::I32),
        other => panic!("expected Optional, got {other:?}"),
    }
    match &fields[2].ty {
        LaicType::Map(k, v) => {
            assert_eq!(**k, LaicType::String);
            assert_eq!(**v, LaicType::F64);
        }
        other => panic!("expected Map, got {other:?}"),
    }
}

#[test]
fn test_parse_error_invalid_syntax() {
    let result = compile("not valid laic");
    assert!(result.is_err());
}

#[test]
fn test_compile_rejects_nested_list() {
    let src = r#"
        version "1.0.0";
        skill test {
            id = "test";
            input TestInput { x: list<list<i32>>; }
            output TestOutput { y: i32; }
        }
    "#;
    let err = compile(src).unwrap_err().to_string();
    assert!(err.contains("nested list<list<T>>"), "{err}");
}

#[test]
fn test_parse_map_various_value_types() {
    let src = r#"
        version "1.0.0";
        skill test {
            id = "test";
            input TestInput {
                a: map<string, string>;
                b: map<string, f64>;
                c: map<i32, bool>;
                d: map<string, bytes>;
            }
            output TestOutput { x: i32; }
        }
    "#;
    let file = compile(src).unwrap_or_else(|e| panic!("compile failed: {e}"));
    let fields = &file.skills[0].input.fields;
    assert_eq!(fields.len(), 4);
    assert!(
        matches!(&fields[0].ty, LaicType::Map(k, v) if **k == LaicType::String && **v == LaicType::String)
    );
    assert!(
        matches!(&fields[1].ty, LaicType::Map(k, v) if **k == LaicType::String && **v == LaicType::F64)
    );
    assert!(
        matches!(&fields[2].ty, LaicType::Map(k, v) if **k == LaicType::I32 && **v == LaicType::Bool)
    );
    assert!(
        matches!(&fields[3].ty, LaicType::Map(k, v) if **k == LaicType::String && **v == LaicType::Bytes)
    );
}
