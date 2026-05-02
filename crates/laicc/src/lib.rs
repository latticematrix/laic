//! LAICC — LAIC IDL compiler.
//!
//! Compiles `.laic` interface definitions into Rust, Python, and
//! TypeScript contract bindings with Arrow IPC serialization.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(missing_docs)]
#![warn(clippy::pedantic)]

pub mod ast;
#[allow(
    clippy::format_push_string,
    clippy::uninlined_format_args,
    clippy::single_match_else,
    clippy::ref_option,
    clippy::match_same_arms,
    clippy::doc_markdown,
    clippy::must_use_candidate,
    clippy::trivially_copy_pass_by_ref,
    clippy::useless_format,
    clippy::missing_errors_doc
)]
pub mod codegen;
pub mod error;
#[allow(missing_docs)]
pub mod parser;
pub mod validate;

use error::CompileError;

/// Parse and validate a `.laic` source string.
///
/// # Errors
///
/// Returns `CompileError::Parse` on syntax errors, `CompileError::Validation` on semantic errors.
pub fn compile(source: &str) -> Result<ast::LaicFile, CompileError> {
    let file = parser::parse(source)?;
    validate::validate(&file)?;
    Ok(file)
}

/// Generate Rust source code from a validated [`ast::LaicFile`].
///
/// # Errors
///
/// Returns `CompileError::Codegen` if code generation fails.
pub fn generate_rust(file: &ast::LaicFile) -> Result<String, CompileError> {
    codegen::rust_contract::generate_rust(file)
}

/// Generate Python source code from a validated [`ast::LaicFile`].
///
/// # Errors
///
/// Returns `CompileError::Codegen` if code generation fails.
pub fn generate_python(file: &ast::LaicFile) -> Result<String, CompileError> {
    codegen::python_contract::generate_python(file)
}

/// Generate TypeScript source code from a validated [`ast::LaicFile`].
///
/// # Errors
///
/// Returns `CompileError::Codegen` if code generation fails.
pub fn generate_typescript(file: &ast::LaicFile) -> Result<String, CompileError> {
    codegen::typescript_contract::generate_typescript(file)
}
