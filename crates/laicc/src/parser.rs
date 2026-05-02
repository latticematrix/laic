//! Parser: `.laic` source → AST.

use pest::Parser;
use pest_derive::Parser;

use crate::ast::{
    Dimension, ErrorVariant, FieldDef, LaicFile, LaicType, Literal, SkillDef, StructDef,
    TensorElementType,
};
use crate::error::CompileError;

#[allow(missing_docs)]
#[derive(Parser)]
#[grammar = "laic.pest"]
struct LaicParser;

/// Parse a `.laic` source string into a [`LaicFile`] AST.
///
/// # Errors
///
/// Returns `CompileError::Parse` if the source has syntax errors.
pub fn parse(source: &str) -> Result<LaicFile, CompileError> {
    let mut pairs =
        LaicParser::parse(Rule::file, source).map_err(|e| CompileError::Parse(format!("{e}")))?;

    let file_pair = pairs
        .next()
        .ok_or_else(|| CompileError::Parse("empty parse result".into()))?;
    parse_file(file_pair)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the next pair or return a parse error.
fn require_next<'a>(
    iter: &mut impl Iterator<Item = pest::iterators::Pair<'a, Rule>>,
    context: &str,
) -> Result<pest::iterators::Pair<'a, Rule>, CompileError> {
    iter.next()
        .ok_or_else(|| CompileError::Parse(format!("expected {context}")))
}

/// Strip surrounding double-quotes from a string literal token.
fn strip_quotes(s: &str) -> String {
    s.trim_matches('"').to_string()
}

/// Normalize physical line endings inside string literals to logical newline semantics.
fn normalize_string_literal_newlines(s: &str) -> String {
    // WHY: `.laic` multiline defaults should keep the same logical text regardless of whether
    // the working tree checked the source file out as LF or CRLF. Preserve content, but collapse
    // physical line-ending representation before language-specific codegen escaping happens.
    s.replace("\r\n", "\n").replace('\r', "\n")
}

// ---------------------------------------------------------------------------
// File
// ---------------------------------------------------------------------------

fn parse_file(pair: pest::iterators::Pair<'_, Rule>) -> Result<LaicFile, CompileError> {
    let mut version = String::new();
    let mut skills = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::version_decl => {
                version = parse_version_decl(inner)?;
            }
            Rule::skill_def => {
                skills.push(parse_skill_def(inner)?);
            }
            _ => {}
        }
    }

    Ok(LaicFile { version, skills })
}

fn parse_version_decl(pair: pest::iterators::Pair<'_, Rule>) -> Result<String, CompileError> {
    let inner = require_next(&mut pair.into_inner(), "version string literal")?;
    Ok(strip_quotes(inner.as_str()))
}

// ---------------------------------------------------------------------------
// Skill
// ---------------------------------------------------------------------------

fn parse_skill_def(pair: pest::iterators::Pair<'_, Rule>) -> Result<SkillDef, CompileError> {
    let mut inner = pair.into_inner();

    let name = require_next(&mut inner, "skill name")?.as_str().to_string();
    let id = parse_id_decl(require_next(&mut inner, "id declaration")?)?;
    let input = parse_struct_def(require_next(&mut inner, "input definition")?)?;
    let output = parse_struct_def(require_next(&mut inner, "output definition")?)?;

    let errors = match inner.next() {
        Some(p) if p.as_rule() == Rule::error_def => parse_error_def(p)?,
        _ => Vec::new(),
    };

    Ok(SkillDef {
        name,
        id,
        input,
        output,
        errors,
    })
}

fn parse_id_decl(pair: pest::iterators::Pair<'_, Rule>) -> Result<String, CompileError> {
    let inner = require_next(&mut pair.into_inner(), "id string literal")?;
    Ok(strip_quotes(inner.as_str()))
}

// ---------------------------------------------------------------------------
// Struct / Field
// ---------------------------------------------------------------------------

fn parse_struct_def(pair: pest::iterators::Pair<'_, Rule>) -> Result<StructDef, CompileError> {
    let mut inner = pair.into_inner();
    let name = require_next(&mut inner, "struct name")?
        .as_str()
        .to_string();
    let mut fields = Vec::new();

    for field_pair in inner {
        if field_pair.as_rule() == Rule::field_def {
            fields.push(parse_field_def(field_pair)?);
        }
    }

    Ok(StructDef { name, fields })
}

fn parse_field_def(pair: pest::iterators::Pair<'_, Rule>) -> Result<FieldDef, CompileError> {
    let mut inner = pair.into_inner();

    let name = require_next(&mut inner, "field name")?.as_str().to_string();
    let ty = parse_laic_type(require_next(&mut inner, "field type")?)?;

    let default = match inner.next() {
        Some(p) if p.as_rule() == Rule::default_value => {
            let lit_pair = require_next(&mut p.into_inner(), "default literal")?;
            Some(parse_literal(lit_pair)?)
        }
        _ => None,
    };

    Ok(FieldDef { name, ty, default })
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

fn parse_laic_type(pair: pest::iterators::Pair<'_, Rule>) -> Result<LaicType, CompileError> {
    // laic_type rule contains either a compound type or a keyword
    let text = pair.as_str().trim();

    // Check for compound types first (they have inner pairs)
    let mut inner = pair.into_inner();
    if let Some(child) = inner.next() {
        return match child.as_rule() {
            Rule::map_type => parse_map_type(child),
            Rule::list_type => parse_list_type(child),
            Rule::optional_type => parse_optional_type(child),
            Rule::tensor_type => parse_tensor_type(child),
            _ => Err(CompileError::Parse(format!(
                "unexpected type rule: {:?}",
                child.as_rule()
            ))),
        };
    }

    // Scalar keyword
    match text {
        "string" => Ok(LaicType::String),
        "bytes" => Ok(LaicType::Bytes),
        "bool" => Ok(LaicType::Bool),
        "i8" => Ok(LaicType::I8),
        "i16" => Ok(LaicType::I16),
        "i32" => Ok(LaicType::I32),
        "i64" => Ok(LaicType::I64),
        "u8" => Ok(LaicType::U8),
        "f32" => Ok(LaicType::F32),
        "f64" => Ok(LaicType::F64),
        other => Err(CompileError::Parse(format!("unknown type: '{other}'"))),
    }
}

fn parse_map_type(pair: pest::iterators::Pair<'_, Rule>) -> Result<LaicType, CompileError> {
    let mut inner = pair.into_inner();
    let key = parse_laic_type(require_next(&mut inner, "map key type")?)?;
    let value = parse_laic_type(require_next(&mut inner, "map value type")?)?;
    Ok(LaicType::Map(Box::new(key), Box::new(value)))
}

fn parse_list_type(pair: pest::iterators::Pair<'_, Rule>) -> Result<LaicType, CompileError> {
    let mut inner = pair.into_inner();
    let elem = parse_laic_type(require_next(&mut inner, "list element type")?)?;
    Ok(LaicType::List(Box::new(elem)))
}

fn parse_optional_type(pair: pest::iterators::Pair<'_, Rule>) -> Result<LaicType, CompileError> {
    let mut inner = pair.into_inner();
    let elem = parse_laic_type(require_next(&mut inner, "optional inner type")?)?;
    Ok(LaicType::Optional(Box::new(elem)))
}

fn parse_tensor_type(pair: pest::iterators::Pair<'_, Rule>) -> Result<LaicType, CompileError> {
    let mut inner = pair.into_inner();
    let dtype = parse_tensor_dtype(require_next(&mut inner, "tensor dtype")?)?;
    let dim_list = require_next(&mut inner, "tensor dimensions")?;
    let dims = parse_dim_list(dim_list)?;
    Ok(LaicType::Tensor { dtype, dims })
}

#[allow(clippy::needless_pass_by_value)]
fn parse_tensor_dtype(
    pair: pest::iterators::Pair<'_, Rule>,
) -> Result<TensorElementType, CompileError> {
    match pair.as_str().trim() {
        "f32" => Ok(TensorElementType::F32),
        "f64" => Ok(TensorElementType::F64),
        "i8" => Ok(TensorElementType::I8),
        "i16" => Ok(TensorElementType::I16),
        "i32" => Ok(TensorElementType::I32),
        "i64" => Ok(TensorElementType::I64),
        "u8" => Ok(TensorElementType::U8),
        "bool" => Ok(TensorElementType::Bool),
        other => Err(CompileError::Parse(format!(
            "unknown tensor dtype: '{other}'"
        ))),
    }
}

fn parse_dim_list(pair: pest::iterators::Pair<'_, Rule>) -> Result<Vec<Dimension>, CompileError> {
    let mut dims = Vec::new();
    for dim_pair in pair.into_inner() {
        if dim_pair.as_rule() == Rule::dimension {
            dims.push(parse_dimension(dim_pair)?);
        }
    }
    Ok(dims)
}

fn parse_dimension(pair: pest::iterators::Pair<'_, Rule>) -> Result<Dimension, CompileError> {
    let text = pair.as_str().trim();
    if text == "_" {
        return Ok(Dimension::Dynamic(None));
    }
    // Check inner rule
    if let Some(child) = pair.into_inner().next() {
        return match child.as_rule() {
            Rule::integer => {
                let val: usize = child
                    .as_str()
                    .parse()
                    .map_err(|e| CompileError::Parse(format!("invalid dimension integer: {e}")))?;
                Ok(Dimension::Fixed(val))
            }
            Rule::ident => Ok(Dimension::Dynamic(Some(child.as_str().to_string()))),
            _ => Err(CompileError::Parse(format!(
                "unexpected dimension rule: {:?}",
                child.as_rule()
            ))),
        };
    }
    Err(CompileError::Parse(format!(
        "could not parse dimension: '{text}'"
    )))
}

// ---------------------------------------------------------------------------
// Error variants
// ---------------------------------------------------------------------------

fn parse_error_def(
    pair: pest::iterators::Pair<'_, Rule>,
) -> Result<Vec<ErrorVariant>, CompileError> {
    let mut variants = Vec::new();
    for child in pair.into_inner() {
        if child.as_rule() == Rule::error_variant {
            variants.push(parse_error_variant(child)?);
        }
    }
    Ok(variants)
}

fn parse_error_variant(
    pair: pest::iterators::Pair<'_, Rule>,
) -> Result<ErrorVariant, CompileError> {
    let mut inner = pair.into_inner();
    let name = require_next(&mut inner, "error variant name")?
        .as_str()
        .to_string();
    let code_str = require_next(&mut inner, "error variant code")?.as_str();
    let code: u16 = code_str
        .parse()
        .map_err(|e| CompileError::Parse(format!("invalid error code '{code_str}': {e}")))?;
    Ok(ErrorVariant { name, code })
}

// ---------------------------------------------------------------------------
// Literals
// ---------------------------------------------------------------------------

fn parse_literal(pair: pest::iterators::Pair<'_, Rule>) -> Result<Literal, CompileError> {
    let child = require_next(&mut pair.into_inner(), "literal value")?;
    match child.as_rule() {
        Rule::string_literal => Ok(Literal::String(normalize_string_literal_newlines(
            &strip_quotes(child.as_str()),
        ))),
        Rule::float_literal => {
            let val: f64 = child
                .as_str()
                .parse()
                .map_err(|e| CompileError::Parse(format!("invalid float literal: {e}")))?;
            Ok(Literal::Float(val))
        }
        Rule::integer_literal => {
            let val: i64 = child
                .as_str()
                .parse()
                .map_err(|e| CompileError::Parse(format!("invalid integer literal: {e}")))?;
            Ok(Literal::Integer(val))
        }
        Rule::bool_literal => match child.as_str() {
            "true" => Ok(Literal::Bool(true)),
            "false" => Ok(Literal::Bool(false)),
            other => Err(CompileError::Parse(format!(
                "invalid bool literal: '{other}'"
            ))),
        },
        _ => Err(CompileError::Parse(format!(
            "unexpected literal rule: {:?}",
            child.as_rule()
        ))),
    }
}
