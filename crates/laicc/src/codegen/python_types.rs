//! `LaicType` → Python type annotation and PyArrow type expression mapping.

use crate::ast::LaicType;

/// Map a LAIC type to its Python type annotation string.
#[must_use]
pub fn python_type(ty: &LaicType) -> String {
    match ty {
        LaicType::String => "str".into(),
        LaicType::Bytes => "bytes".into(),
        LaicType::Bool => "bool".into(),
        LaicType::I8 | LaicType::I16 | LaicType::I32 | LaicType::I64 | LaicType::U8 => "int".into(),
        LaicType::F32 | LaicType::F64 => "float".into(),
        LaicType::Tensor { .. } => "bytes".into(),
        LaicType::List(inner) => format!("list[{}]", python_type(inner)),
        LaicType::Optional(inner) => format!("{} | None", python_type(inner)),
        LaicType::Map(k, v) => format!("dict[{}, {}]", python_type(k), python_type(v)),
    }
}

/// Map a LAIC type to its PyArrow type constructor expression.
#[must_use]
pub fn pyarrow_type(ty: &LaicType) -> String {
    match ty {
        LaicType::String => "pa.string()".into(),
        LaicType::Bytes => "pa.binary()".into(),
        LaicType::Bool => "pa.bool_()".into(),
        LaicType::I8 => "pa.int8()".into(),
        LaicType::I16 => "pa.int16()".into(),
        LaicType::I32 => "pa.int32()".into(),
        LaicType::I64 => "pa.int64()".into(),
        LaicType::U8 => "pa.uint8()".into(),
        LaicType::F32 => "pa.float32()".into(),
        LaicType::F64 => "pa.float64()".into(),
        LaicType::Tensor { .. } => "pa.binary()".into(),
        LaicType::List(inner) => format!("pa.list_({})", pyarrow_type(inner)),
        LaicType::Optional(inner) => pyarrow_type(inner),
        LaicType::Map(k, v) => format!("pa.map_({}, {})", pyarrow_type(k), pyarrow_type(v)),
    }
}

/// Map a LAIC literal default value to Python source code.
pub fn literal_to_python(lit: &crate::ast::Literal) -> String {
    match lit {
        crate::ast::Literal::String(s) => format!(
            "\"{}\"",
            crate::codegen::escape_string_literal_body(
                s,
                crate::codegen::StringLiteralDialect::PythonLike
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
        crate::ast::Literal::Bool(b) => {
            if *b {
                "True".into()
            } else {
                "False".into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::TensorElementType;

    #[test]
    fn scalar_python_types() {
        assert_eq!(python_type(&LaicType::String), "str");
        assert_eq!(python_type(&LaicType::I32), "int");
        assert_eq!(python_type(&LaicType::F64), "float");
        assert_eq!(python_type(&LaicType::Bytes), "bytes");
        assert_eq!(python_type(&LaicType::Bool), "bool");
    }

    #[test]
    fn container_python_types() {
        assert_eq!(
            python_type(&LaicType::List(Box::new(LaicType::String))),
            "list[str]"
        );
        assert_eq!(
            python_type(&LaicType::Optional(Box::new(LaicType::I32))),
            "int | None"
        );
        assert_eq!(
            python_type(&LaicType::Map(
                Box::new(LaicType::String),
                Box::new(LaicType::F64)
            )),
            "dict[str, float]"
        );
    }

    #[test]
    fn scalar_pyarrow_types() {
        assert_eq!(pyarrow_type(&LaicType::String), "pa.string()");
        assert_eq!(pyarrow_type(&LaicType::Bool), "pa.bool_()");
        assert_eq!(pyarrow_type(&LaicType::I32), "pa.int32()");
        assert_eq!(pyarrow_type(&LaicType::F64), "pa.float64()");
    }

    #[test]
    fn tensor_python_type() {
        let ty = LaicType::Tensor {
            dtype: TensorElementType::F32,
            dims: vec![crate::ast::Dimension::Fixed(768)],
        };
        assert_eq!(python_type(&ty), "bytes");
        assert_eq!(pyarrow_type(&ty), "pa.binary()");
    }
}
