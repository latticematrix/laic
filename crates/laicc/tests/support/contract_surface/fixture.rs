use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuntimeConfig {
    pub(crate) input_class: &'static str,
    pub(crate) output_class: &'static str,
    pub(crate) input_fields: &'static [&'static str],
    pub(crate) output_fields: &'static [&'static str],
    pub(crate) input_args: Vec<Value>,
    pub(crate) output_args: Vec<Value>,
    pub(crate) error_enum: Option<&'static str>,
    pub(crate) tensor_field: Option<&'static str>,
    pub(crate) bad_tensor_dtype: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub(crate) struct FixtureSpec {
    pub(crate) stem: &'static str,
    pub(crate) runtime: RuntimeConfig,
    pub(crate) expected_input: BTreeMap<String, Value>,
    pub(crate) expected_output: BTreeMap<String, Value>,
}

pub(crate) fn fixture_spec(fixture: &str) -> FixtureSpec {
    match fixture.trim_end_matches(".laic") {
        "echo" => FixtureSpec {
            stem: "echo",
            runtime: RuntimeConfig {
                input_class: "EchoInput",
                output_class: "EchoOutput",
                input_fields: &["text"],
                output_fields: &["text"],
                input_args: vec![json!("hello world")],
                output_args: vec![json!("echo: hello")],
                error_enum: None,
                tensor_field: None,
                bad_tensor_dtype: None,
            },
            expected_input: fields([("text", json!("hello world"))]),
            expected_output: fields([("text", json!("echo: hello"))]),
        },
        "escaped_defaults" => FixtureSpec {
            stem: "escaped_defaults",
            runtime: RuntimeConfig {
                input_class: "EscapedDefaultsInput",
                output_class: "EscapedDefaultsOutput",
                input_fields: &["message", "path"],
                output_fields: &["note"],
                input_args: vec![],
                output_args: vec![],
                error_enum: None,
                tensor_field: None,
                bad_tensor_dtype: None,
            },
            expected_input: fields([
                ("message", json!("line1\nline2")),
                ("path", json!(r"C:\temp\file.txt")),
            ]),
            expected_output: fields([("note", json!("line1\nline2"))]),
        },
        "embedding" => FixtureSpec {
            stem: "embedding",
            runtime: RuntimeConfig {
                input_class: "EmbeddingInput",
                output_class: "EmbeddingOutput",
                input_fields: &["text", "model", "max_tokens"],
                output_fields: &["embedding", "token_count"],
                input_args: vec![json!("hello")],
                output_args: vec![json!({"__bytes_len__": 768 * 4}), json!(42)],
                error_enum: Some("EmbeddingError"),
                tensor_field: Some("embedding"),
                bad_tensor_dtype: Some("f64"),
            },
            expected_input: fields([
                ("text", json!("hello")),
                ("model", json!("default")),
                ("max_tokens", json!(512)),
            ]),
            expected_output: fields([
                ("embedding", json!({"kind": "bytes", "len": 768 * 4})),
                ("token_count", json!(42)),
            ]),
        },
        "optional_types" => FixtureSpec {
            stem: "optional_types",
            runtime: RuntimeConfig {
                input_class: "SearchInput",
                output_class: "SearchOutput",
                input_fields: &["query", "max_results", "filter_tag", "threshold"],
                output_fields: &["results", "total_count", "next_cursor"],
                input_args: vec![json!("hello"), Value::Null, Value::Null, Value::Null],
                output_args: vec![json!(["a", "b"]), json!(2), Value::Null],
                error_enum: Some("SearchError"),
                tensor_field: None,
                bad_tensor_dtype: None,
            },
            expected_input: fields([
                ("query", json!("hello")),
                ("max_results", Value::Null),
                ("filter_tag", Value::Null),
                ("threshold", Value::Null),
            ]),
            expected_output: fields([
                ("results", json!(["a", "b"])),
                ("total_count", json!(2)),
                ("next_cursor", Value::Null),
            ]),
        },
        other => panic!("unsupported contract-surface fixture: {other}"),
    }
}

fn fields<const N: usize>(pairs: [(&str, Value); N]) -> BTreeMap<String, Value> {
    pairs
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}
