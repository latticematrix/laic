//! Code generation modules.
//!
//! WHY: codegen modules use `push_str(&format!(...))` extensively - this is the natural
//! pattern for source code generation.

pub mod python_contract;
pub mod python_deserialize;
pub mod python_serialize;
pub mod python_types;
pub mod rust_contract;
pub mod rust_deserialize;
pub mod rust_serialize;
pub mod rust_types;
// Keep the TypeScript split parallel to the Python path so future maintainers can
// compare language targets module-by-module instead of diffing one giant generator.
pub mod typescript_contract;
pub mod typescript_deserialize;
pub mod typescript_serialize;
pub mod typescript_types;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StringLiteralDialect {
    PythonLike,
    Rust,
}

/// Convert `SCREAMING_SNAKE_CASE` or `snake_case` to `PascalCase`.
pub(crate) fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
            }
        })
        .collect()
}

pub(crate) fn escape_string_literal_body(value: &str, dialect: StringLiteralDialect) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\0' => match dialect {
                // WHY: Python and TypeScript both treat `\01` ambiguously or invalidly when a
                // digit follows the NUL escape. Use a fixed-width form so emitted defaults keep
                // the exact `NUL + next char` semantics.
                StringLiteralDialect::PythonLike => escaped.push_str("\\x00"),
                StringLiteralDialect::Rust => escaped.push_str("\\0"),
            },
            '\u{2028}' => match dialect {
                StringLiteralDialect::PythonLike => escaped.push_str("\\u2028"),
                StringLiteralDialect::Rust => escaped.push_str("\\u{2028}"),
            },
            '\u{2029}' => match dialect {
                StringLiteralDialect::PythonLike => escaped.push_str("\\u2029"),
                StringLiteralDialect::Rust => escaped.push_str("\\u{2029}"),
            },
            ch if ch.is_control() => {
                let code = ch as u32;
                match dialect {
                    StringLiteralDialect::PythonLike => {
                        if code <= 0xFF {
                            escaped.push_str(&format!("\\x{code:02X}"));
                        } else if code <= 0xFFFF {
                            escaped.push_str(&format!("\\u{code:04X}"));
                        } else {
                            escaped.push_str(&format!("\\U{code:08X}"));
                        }
                    }
                    StringLiteralDialect::Rust => {
                        if code <= 0xFF {
                            escaped.push_str(&format!("\\x{code:02X}"));
                        } else {
                            escaped.push_str(&format!("\\u{{{code:X}}}"));
                        }
                    }
                }
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

pub(crate) fn rust_string_literal(value: &str) -> String {
    format!(
        "\"{}\"",
        escape_string_literal_body(value, StringLiteralDialect::Rust)
    )
}

pub(crate) fn python_string_literal(value: &str) -> String {
    format!(
        "\"{}\"",
        escape_string_literal_body(value, StringLiteralDialect::PythonLike)
    )
}

pub(crate) fn python_bytes_literal(value: &str) -> String {
    // WHY: PyArrow metadata values are bytes, but `b"..."` rejects non-ASCII source.
    // Emit a UTF-8 encode expression so metadata escaping follows normal Python strings.
    format!("{}.encode(\"utf-8\")", python_string_literal(value))
}

pub(crate) fn typescript_string_literal(value: &str) -> String {
    python_string_literal(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("INPUT_TOO_LONG"), "InputTooLong");
        assert_eq!(to_pascal_case("MODEL_NOT_FOUND"), "ModelNotFound");
        assert_eq!(to_pascal_case("TIMEOUT"), "Timeout");
        assert_eq!(to_pascal_case("echo"), "Echo");
        assert_eq!(to_pascal_case("health_check"), "HealthCheck");
        assert_eq!(to_pascal_case("image_classify"), "ImageClassify");
    }

    #[test]
    fn escape_python_like_string_literal_body() {
        assert_eq!(
            escape_string_literal_body("line1\nline2", StringLiteralDialect::PythonLike),
            "line1\\nline2"
        );
        assert_eq!(
            escape_string_literal_body(r"C:\temp\file.txt", StringLiteralDialect::PythonLike),
            r"C:\\temp\\file.txt"
        );
        assert_eq!(
            escape_string_literal_body(
                &format!("before{}\u{0031}after", '\0'),
                StringLiteralDialect::PythonLike
            ),
            r"before\x001after"
        );
    }

    #[test]
    fn escape_rust_string_literal_body() {
        assert_eq!(
            escape_string_literal_body("line1\nline2", StringLiteralDialect::Rust),
            "line1\\nline2"
        );
        assert_eq!(
            escape_string_literal_body(r"C:\temp\file.txt", StringLiteralDialect::Rust),
            r"C:\\temp\\file.txt"
        );
    }
}
