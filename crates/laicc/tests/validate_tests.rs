//! Validator unit tests.

#![allow(clippy::unwrap_used)]

use laicc::CompileError;

fn parse_and_validate(src: &str) -> Result<(), CompileError> {
    laicc::compile(src).map(|_| ())
}

#[test]
fn valid_minimal() {
    let src = r#"
        version "1.0.0";
        skill echo {
            id = "echo";
            input EchoInput { text: string; }
            output EchoOutput { text: string; }
        }
    "#;
    parse_and_validate(src).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn reject_duplicate_skill_name() {
    let src = r#"
        version "1.0.0";
        skill echo { id = "a"; input A { x: i32; } output B { y: i32; } }
        skill echo { id = "b"; input C { x: i32; } output D { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("duplicate skill name"), "{err}");
}

#[test]
fn reject_duplicate_skill_id() {
    let src = r#"
        version "1.0.0";
        skill a { id = "same"; input A { x: i32; } output B { y: i32; } }
        skill b { id = "same"; input C { x: i32; } output D { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("duplicate skill id"), "{err}");
}

#[test]
fn reject_duplicate_field() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: i32; x: string; } output O { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("duplicate field 'x'"), "{err}");
}

#[test]
fn reject_empty_input() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I {} output O { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("at least one field"), "{err}");
}

#[test]
fn reject_duplicate_error_code() {
    let src = r#"
        version "1.0.0";
        skill t {
            id = "t";
            input I { x: i32; }
            output O { y: i32; }
            error { A = 1; B = 1; }
        }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("duplicate error code"), "{err}");
}

#[test]
fn reject_duplicate_error_name() {
    let src = r#"
        version "1.0.0";
        skill t {
            id = "t";
            input I { x: i32; }
            output O { y: i32; }
            error { A = 1; A = 2; }
        }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("duplicate error name"), "{err}");
}

#[test]
fn reject_error_code_zero() {
    let src = r#"
        version "1.0.0";
        skill t {
            id = "t";
            input I { x: i32; }
            output O { y: i32; }
            error { A = 0; }
        }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("must be positive"), "{err}");
}

#[test]
fn reject_incompatible_default() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: i32 = "hello"; } output O { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("incompatible default"), "{err}");
}

#[test]
fn accept_integer_default_for_float() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: f64 = 42; } output O { y: i32; } }
    "#;
    parse_and_validate(src).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn reject_reserved_codegen_field_identifier() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { type: string; } output O { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("reserved codegen identifier"), "{err}");
}

#[test]
fn reject_reserved_codegen_struct_identifier() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input type { x: string; } output O { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("reserved codegen identifier"), "{err}");
}

#[test]
fn reject_reserved_codegen_error_identifier() {
    let src = r#"
        version "1.0.0";
        skill t {
            id = "t";
            input I { x: i32; }
            output O { y: i32; }
            error { class = 1; }
        }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("reserved codegen identifier"), "{err}");
}

#[test]
fn reject_typescript_restricted_binding_identifier() {
    for name in ["arguments", "eval"] {
        let src = format!(
            r#"
            version "1.0.0";
            skill t {{ id = "t"; input I {{ {name}: string; }} output O {{ y: i32; }} }}
            "#
        );
        let err = parse_and_validate(&src).unwrap_err().to_string();
        assert!(err.contains("reserved codegen identifier"), "{err}");
    }
}

#[test]
fn reject_integer_default_outside_concrete_width() {
    for (ty, value) in [("u8", "-1"), ("u8", "256"), ("i8", "128"), ("i16", "32768")] {
        let src = format!(
            r#"
            version "1.0.0";
            skill t {{ id = "t"; input I {{ x: {ty} = {value}; }} output O {{ y: i32; }} }}
            "#
        );
        let err = parse_and_validate(&src).unwrap_err().to_string();
        assert!(err.contains("out of range"), "{err}");
    }
}

#[test]
fn accept_integer_default_at_concrete_width_boundaries() {
    for (ty, value) in [
        ("u8", "0"),
        ("u8", "255"),
        ("i8", "-128"),
        ("i8", "127"),
        ("i16", "-32768"),
        ("i16", "32767"),
    ] {
        let src = format!(
            r#"
            version "1.0.0";
            skill t {{ id = "t"; input I {{ x: {ty} = {value}; }} output O {{ y: i32; }} }}
            "#
        );
        parse_and_validate(&src).unwrap_or_else(|e| panic!("{ty}={value}: {e}"));
    }
}

#[test]
fn reject_nested_list() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: list<list<i32>>; } output O { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("nested list<list<T>>"), "{err}");
}

#[test]
fn reject_nested_optional() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: optional<optional<i32>>; } output O { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("nested optional<optional<T>>"), "{err}");
}

#[test]
fn reject_map_float_key() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: map<f32, string>; } output O { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("map key must be"), "{err}");
}

#[test]
fn reject_map_complex_value() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: map<string, list<i32>>; } output O { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("map value must be"), "{err}");
}

#[test]
fn reject_list_tensor_dynamic() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: list<tensor<f32>[_, 768]>; } output O { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("dynamic dimensions"), "{err}");
}

#[test]
fn reject_fixed_tensor_dimension_zero() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: tensor<f32>[0]; } output O { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("dimension 0"), "{err}");
}

#[test]
fn accept_list_tensor_fixed() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: list<tensor<f32>[768]>; } output O { y: i32; } }
    "#;
    parse_and_validate(src).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn reject_list_of_map() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: list<map<string, i32>>; } output O { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("list<map<...>>"), "{err}");
}

#[test]
fn reject_optional_of_map() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: optional<map<string, i32>>; } output O { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err().to_string();
    assert!(err.contains("optional<map<...>>"), "{err}");
}

#[test]
fn accept_valid_map_types() {
    let src = r#"
        version "1.0.0";
        skill t {
            id = "t";
            input I { a: map<string, string>; b: map<i32, bool>; c: map<string, f64>; }
            output O { y: i32; }
        }
    "#;
    parse_and_validate(src).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn accept_list_of_optional() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: list<optional<string>>; } output O { y: i32; } }
    "#;
    parse_and_validate(src).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn accept_optional_of_list() {
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: optional<list<i32>>; } output O { y: i32; } }
    "#;
    parse_and_validate(src).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn reject_list_optional_nested_container() {
    // list<optional<list<i32>>> — optional inner is not a leaf type
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: list<optional<list<i32>>>; } output O { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err();
    assert!(
        err.to_string()
            .contains("list<optional<T>> requires T to be"),
        "unexpected error: {err}"
    );
}

#[test]
fn reject_optional_list_nested_container() {
    // optional<list<optional<string>>> — list inner is not a leaf type
    let src = r#"
        version "1.0.0";
        skill t { id = "t"; input I { x: optional<list<optional<string>>>; } output O { y: i32; } }
    "#;
    let err = parse_and_validate(src).unwrap_err();
    assert!(
        err.to_string()
            .contains("optional<list<T>> requires T to be"),
        "unexpected error: {err}"
    );
}
