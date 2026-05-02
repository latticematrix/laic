//! Compiler error types.

use std::fmt;

/// Errors produced by the laicc compiler pipeline.
#[derive(Debug)]
pub enum CompileError {
    /// PEG parse failure.
    Parse(String),
    /// Semantic validation failure.
    Validation(String),
    /// Code generation failure.
    Codegen(String),
    /// File I/O failure.
    Io(std::io::Error),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(msg) => write!(f, "parse error: {msg}"),
            Self::Validation(msg) => write!(f, "validation error: {msg}"),
            Self::Codegen(msg) => write!(f, "codegen error: {msg}"),
            Self::Io(err) => write!(f, "I/O error: {err}"),
        }
    }
}

impl std::error::Error for CompileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CompileError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}
