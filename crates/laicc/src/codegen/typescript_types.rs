//! TypeScript type mapping helpers.
//!
//! WHY: these mappings are intentionally narrow. When TS cannot represent a LAIC shape
//! cleanly, we prefer an explicit, documented compromise over a larger runtime layer.

use crate::ast::{Dimension, LaicType, Literal};

/// Map a LAIC type to its TypeScript type annotation.
#[must_use]
pub fn ts_type(ty: &LaicType) -> String {
    match ty {
        LaicType::String => "string".into(),
        LaicType::Bytes | LaicType::Tensor { .. } => "Uint8Array".into(),
        LaicType::Bool => "boolean".into(),
        LaicType::I8
        | LaicType::I16
        | LaicType::I32
        | LaicType::U8
        | LaicType::F32
        | LaicType::F64 => "number".into(),
        LaicType::I64 => "bigint".into(),
        LaicType::List(inner) => {
            let inner = ts_type(inner);
            if inner.contains('|') || inner.contains('<') {
                format!("Array<{inner}>")
            } else {
                format!("{inner}[]")
            }
        }
        LaicType::Optional(inner) => format!("{} | null", ts_type(inner)),
        LaicType::Map(key, value) => format!("Map<{}, {}>", ts_type(key), ts_type(value)),
    }
}

/// Map a LAIC type to an Arrow JS datatype constructor.
#[must_use]
pub fn ts_arrow_datatype(ty: &LaicType) -> String {
    match ty {
        LaicType::String => "new arrow.Utf8()".into(),
        LaicType::Bytes | LaicType::Tensor { .. } => "new arrow.Binary()".into(),
        LaicType::Bool => "new arrow.Bool()".into(),
        LaicType::I8 => "new arrow.Int8()".into(),
        LaicType::I16 => "new arrow.Int16()".into(),
        LaicType::I32 => "new arrow.Int32()".into(),
        LaicType::I64 => "new arrow.Int64()".into(),
        LaicType::U8 => "new arrow.Uint8()".into(),
        LaicType::F32 => "new arrow.Float32()".into(),
        LaicType::F64 => "new arrow.Float64()".into(),
        LaicType::List(inner) => {
            let nullable = matches!(inner.as_ref(), LaicType::Optional(_));
            format!(
                "new arrow.List(new arrow.Field(\"item\", {}, {}))",
                ts_arrow_datatype(inner),
                nullable
            )
        }
        LaicType::Optional(inner) => ts_arrow_datatype(inner),
        LaicType::Map(key, value) => format!(
            // Arrow JS exposes `Map_` with a type surface that is stricter than the
            // runtime objects accepted by `vectorFromArray` / `Table`. Keep the
            // compatibility cast confined to generated datatype construction.
            "new arrow.Map_(new arrow.Field(\"entries\", new arrow.Struct([new arrow.Field(\"key\", {}, false) as any, new arrow.Field(\"value\", {}, false) as any]) as any, false), false) as any",
            ts_arrow_datatype(key),
            ts_arrow_datatype(value)
        ),
    }
}

/// Convert a LAIC literal to TypeScript source.
#[must_use]
pub fn literal_to_ts(lit: &Literal) -> String {
    match lit {
        Literal::String(value) => format!(
            "\"{}\"",
            crate::codegen::escape_string_literal_body(
                value,
                crate::codegen::StringLiteralDialect::PythonLike
            )
        ),
        Literal::Integer(value) => value.to_string(),
        Literal::Float(value) => {
            let rendered = format!("{value}");
            if rendered.contains('.') {
                rendered
            } else {
                format!("{rendered}.0")
            }
        }
        Literal::Bool(value) => value.to_string(),
    }
}

/// Render tensor dimensions as a metadata string.
#[must_use]
pub fn format_ts_dims(dims: &[Dimension]) -> String {
    // Dynamic dimensions continue to use `0` as a cross-language metadata sentinel so
    // Rust/Python/TypeScript can compare shape strings without a target-specific DSL.
    let parts: Vec<String> = dims
        .iter()
        .map(|dim| match dim {
            Dimension::Fixed(size) => size.to_string(),
            Dimension::Dynamic(_) => "0".into(),
        })
        .collect();
    format!("[{}]", parts.join(","))
}
