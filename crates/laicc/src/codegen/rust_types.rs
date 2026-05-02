//! `LaicType` → Rust type and Arrow `DataType` mapping.

use crate::ast::LaicType;

/// Map a LAIC type to its Rust source representation.
#[must_use]
pub fn rust_type(ty: &LaicType) -> String {
    match ty {
        LaicType::String => "String".into(),
        LaicType::Bytes => "Vec<u8>".into(),
        LaicType::Bool => "bool".into(),
        LaicType::I8 => "i8".into(),
        LaicType::I16 => "i16".into(),
        LaicType::I32 => "i32".into(),
        LaicType::I64 => "i64".into(),
        LaicType::U8 => "u8".into(),
        LaicType::F32 => "f32".into(),
        LaicType::F64 => "f64".into(),
        LaicType::Tensor { .. } => "Vec<u8>".into(),
        LaicType::List(inner) => format!("Vec<{}>", rust_type(inner)),
        LaicType::Optional(inner) => format!("Option<{}>", rust_type(inner)),
        LaicType::Map(k, v) => format!("HashMap<{}, {}>", rust_type(k), rust_type(v)),
    }
}

/// Map a LAIC type to its Arrow DataType constructor expression (as source code).
pub fn arrow_datatype(ty: &LaicType) -> String {
    match ty {
        LaicType::String => "DataType::Utf8".into(),
        LaicType::Bytes => "DataType::Binary".into(),
        LaicType::Bool => "DataType::Boolean".into(),
        LaicType::I8 => "DataType::Int8".into(),
        LaicType::I16 => "DataType::Int16".into(),
        LaicType::I32 => "DataType::Int32".into(),
        LaicType::I64 => "DataType::Int64".into(),
        LaicType::U8 => "DataType::UInt8".into(),
        LaicType::F32 => "DataType::Float32".into(),
        LaicType::F64 => "DataType::Float64".into(),
        LaicType::Tensor { .. } => "DataType::Binary".into(),
        LaicType::List(inner) => {
            // WHY: ListBuilder always produces nullable inner field; schema must match
            format!(
                "DataType::List(Arc::new(Field::new(\"item\", {}, true)))",
                arrow_datatype(inner),
            )
        }
        LaicType::Optional(inner) => arrow_datatype(inner),
        // WHY: MapBuilder produces field names "keys"/"values" with values nullable=true;
        // schema must match builder output exactly
        LaicType::Map(k, v) => {
            format!(
                "DataType::Map(Arc::new(Field::new(\"entries\", DataType::Struct(Fields::from(vec![\
                Field::new(\"keys\", {}, false), \
                Field::new(\"values\", {}, true)\
                ])), false)), false)",
                arrow_datatype(k),
                arrow_datatype(v),
            )
        }
    }
}

/// Map a LAIC type to its Arrow array type name (e.g. "StringArray").
pub fn arrow_array_type(ty: &LaicType) -> &'static str {
    match ty {
        LaicType::String => "StringArray",
        LaicType::Bytes | LaicType::Tensor { .. } => "BinaryArray",
        LaicType::Bool => "BooleanArray",
        LaicType::I8 => "Int8Array",
        LaicType::I16 => "Int16Array",
        LaicType::I32 => "Int32Array",
        LaicType::I64 => "Int64Array",
        LaicType::U8 => "UInt8Array",
        LaicType::F32 => "Float32Array",
        LaicType::F64 => "Float64Array",
        // WHY: validator rejects types that would reach here (nested containers)
        _ => unreachable!("arrow_array_type called with unsupported type: {ty:?}"),
    }
}

/// Map a LAIC type to its Arrow builder type name (e.g. "StringBuilder").
pub fn arrow_builder_type(ty: &LaicType) -> &'static str {
    match ty {
        LaicType::String => "StringBuilder",
        LaicType::Bytes | LaicType::Tensor { .. } => "BinaryBuilder",
        LaicType::Bool => "BooleanBuilder",
        LaicType::I8 => "Int8Builder",
        LaicType::I16 => "Int16Builder",
        LaicType::I32 => "Int32Builder",
        LaicType::I64 => "Int64Builder",
        LaicType::U8 => "UInt8Builder",
        LaicType::F32 => "Float32Builder",
        LaicType::F64 => "Float64Builder",
        // WHY: validator rejects types that would reach here (nested containers)
        _ => unreachable!("arrow_builder_type called with unsupported type: {ty:?}"),
    }
}

/// Whether a value of this type needs `*` dereference when appending to a builder.
pub fn needs_deref(ty: &LaicType) -> bool {
    !matches!(
        ty,
        LaicType::String | LaicType::Bytes | LaicType::Tensor { .. }
    )
}

/// Literal value to Rust source code.
pub fn literal_to_rust(lit: &crate::ast::Literal) -> String {
    match lit {
        crate::ast::Literal::String(s) => format!(
            "\"{}\".to_string()",
            crate::codegen::escape_string_literal_body(
                s,
                crate::codegen::StringLiteralDialect::Rust
            )
        ),
        crate::ast::Literal::Integer(i) => format!("{i}"),
        crate::ast::Literal::Float(f) => {
            let s = format!("{f}");
            if s.contains('.') {
                s
            } else {
                format!("{s}.0")
            }
        }
        crate::ast::Literal::Bool(b) => format!("{b}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::TensorElementType;

    #[test]
    fn scalar_rust_types() {
        assert_eq!(rust_type(&LaicType::String), "String");
        assert_eq!(rust_type(&LaicType::I32), "i32");
        assert_eq!(rust_type(&LaicType::F64), "f64");
        assert_eq!(rust_type(&LaicType::Bytes), "Vec<u8>");
    }

    #[test]
    fn container_rust_types() {
        assert_eq!(
            rust_type(&LaicType::List(Box::new(LaicType::String))),
            "Vec<String>"
        );
        assert_eq!(
            rust_type(&LaicType::Optional(Box::new(LaicType::I32))),
            "Option<i32>"
        );
        assert_eq!(
            rust_type(&LaicType::Map(
                Box::new(LaicType::String),
                Box::new(LaicType::F64)
            )),
            "HashMap<String, f64>"
        );
    }

    #[test]
    fn tensor_rust_type() {
        let ty = LaicType::Tensor {
            dtype: TensorElementType::F32,
            dims: vec![crate::ast::Dimension::Fixed(768)],
        };
        assert_eq!(rust_type(&ty), "Vec<u8>");
    }

    #[test]
    fn scalar_arrow_datatypes() {
        assert_eq!(arrow_datatype(&LaicType::String), "DataType::Utf8");
        assert_eq!(arrow_datatype(&LaicType::Bool), "DataType::Boolean");
        assert_eq!(arrow_datatype(&LaicType::I32), "DataType::Int32");
    }
}
