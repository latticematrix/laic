//! Semantic validation for parsed LAIC AST.

use std::collections::HashSet;

use crate::ast::{Dimension, FieldDef, LaicFile, LaicType, Literal};
use crate::error::CompileError;

/// Validate a parsed [`LaicFile`] against IDL semantic rules.
///
/// # Errors
///
/// Returns `CompileError::Validation` if the AST violates semantic rules.
pub fn validate(file: &LaicFile) -> Result<(), CompileError> {
    if file.version.is_empty() {
        return Err(CompileError::Validation(
            "version string must not be empty".into(),
        ));
    }

    if file.skills.is_empty() {
        return Err(CompileError::Validation(
            "file must contain at least one skill definition".into(),
        ));
    }

    let mut skill_names = HashSet::new();
    let mut skill_ids = HashSet::new();

    for skill in &file.skills {
        if !skill_names.insert(&skill.name) {
            return Err(CompileError::Validation(format!(
                "duplicate skill name: '{}'",
                skill.name
            )));
        }
        if !skill_ids.insert(&skill.id) {
            return Err(CompileError::Validation(format!(
                "duplicate skill id: '{}'",
                skill.id
            )));
        }
        if skill.id.is_empty() {
            return Err(CompileError::Validation(format!(
                "skill '{}': id must not be empty",
                skill.name
            )));
        }

        validate_codegen_identifier(&skill.name, "skill name", &skill.name)?;
        validate_struct(&skill.name, "input", &skill.input)?;
        validate_struct(&skill.name, "output", &skill.output)?;
        validate_errors(&skill.name, &skill.errors)?;
    }

    Ok(())
}

fn validate_struct(
    skill_name: &str,
    direction: &str,
    def: &crate::ast::StructDef,
) -> Result<(), CompileError> {
    if def.name.is_empty() {
        return Err(CompileError::Validation(format!(
            "skill '{skill_name}': {direction} struct name must not be empty"
        )));
    }
    if def.fields.is_empty() {
        return Err(CompileError::Validation(format!(
            "skill '{skill_name}': {direction} struct '{}' must have at least one field",
            def.name
        )));
    }
    validate_codegen_identifier(skill_name, &format!("{direction} struct name"), &def.name)?;

    let mut field_names = HashSet::new();
    for field in &def.fields {
        if !field_names.insert(&field.name) {
            return Err(CompileError::Validation(format!(
                "skill '{skill_name}': duplicate field '{}' in {direction} struct '{}'",
                field.name, def.name
            )));
        }
        validate_field(skill_name, field)?;
    }

    Ok(())
}

fn validate_field(skill_name: &str, field: &FieldDef) -> Result<(), CompileError> {
    validate_codegen_identifier(skill_name, "field name", &field.name)?;
    if let Some(ref default) = field.default {
        validate_default_compat(skill_name, &field.name, &field.ty, default)?;
    }
    validate_field_type(skill_name, &field.name, &field.ty)
}

/// Validate parameterized type constraints.
fn validate_field_type(
    skill_name: &str,
    field_name: &str,
    ty: &LaicType,
) -> Result<(), CompileError> {
    match ty {
        LaicType::Tensor { dims, .. } => validate_tensor_dimensions(skill_name, field_name, dims)?,
        LaicType::List(inner) => validate_list_type_constraints(skill_name, field_name, inner)?,
        LaicType::Optional(inner) => {
            validate_optional_type_constraints(skill_name, field_name, inner)?;
        }
        LaicType::Map(key, value) => {
            validate_map_type_constraints(skill_name, field_name, key, value)?;
        }
        _ => {}
    }
    Ok(())
}

fn validate_tensor_dimensions(
    skill_name: &str,
    field_name: &str,
    dims: &[Dimension],
) -> Result<(), CompileError> {
    if dims.is_empty() {
        return Err(CompileError::Validation(format!(
            "skill '{skill_name}': tensor field '{field_name}' must have at least one dimension"
        )));
    }
    if dims.iter().any(|d| matches!(d, Dimension::Fixed(0))) {
        // WHY: TypeScript uses `0` as the dynamic-dimension metadata sentinel.
        // Allowing a real fixed zero here would make the same schema mean two
        // different things across languages, so reject it at validation time.
        return Err(CompileError::Validation(format!(
            "skill '{skill_name}': tensor field '{field_name}' cannot use fixed dimension 0"
        )));
    }
    Ok(())
}

fn validate_list_type_constraints(
    skill_name: &str,
    field_name: &str,
    inner: &LaicType,
) -> Result<(), CompileError> {
    if matches!(inner, LaicType::List(_)) {
        return Err(CompileError::Validation(format!(
            "skill '{skill_name}': field '{field_name}': nested list<list<T>> is not supported"
        )));
    }
    if let LaicType::Tensor { dims, .. } = inner {
        if dims.iter().any(|d| matches!(d, Dimension::Dynamic(_))) {
            return Err(CompileError::Validation(format!(
                "skill '{skill_name}': field '{field_name}': list<tensor<...>> with dynamic dimensions is not supported"
            )));
        }
    }
    if matches!(inner, LaicType::Map(_, _)) {
        return Err(CompileError::Validation(format!(
            "skill '{skill_name}': field '{field_name}': list<map<...>> is not supported"
        )));
    }
    // WHY: codegen only handles list<optional<leaf>>; deeper nesting
    // (e.g. list<optional<list<T>>>) would produce GenericBuilder.
    if let LaicType::Optional(opt_inner) = inner {
        if !is_leaf_type(opt_inner) {
            return Err(CompileError::Validation(format!(
                "skill '{skill_name}': field '{field_name}': list<optional<T>> requires T to be a scalar, string, bytes, or tensor type"
            )));
        }
    }
    validate_field_type(skill_name, field_name, inner)
}

fn validate_optional_type_constraints(
    skill_name: &str,
    field_name: &str,
    inner: &LaicType,
) -> Result<(), CompileError> {
    if matches!(inner, LaicType::Optional(_)) {
        return Err(CompileError::Validation(format!(
            "skill '{skill_name}': field '{field_name}': nested optional<optional<T>> is not supported"
        )));
    }
    if let LaicType::Tensor { dims, .. } = inner {
        if dims.iter().any(|d| matches!(d, Dimension::Dynamic(_))) {
            return Err(CompileError::Validation(format!(
                "skill '{skill_name}': field '{field_name}': optional<tensor<...>> with dynamic dimensions is not supported"
            )));
        }
    }
    if matches!(inner, LaicType::Map(_, _)) {
        return Err(CompileError::Validation(format!(
            "skill '{skill_name}': field '{field_name}': optional<map<...>> is not supported"
        )));
    }
    // WHY: codegen only handles optional<list<leaf>>; deeper nesting
    // (e.g. optional<list<optional<T>>>) would produce GenericBuilder.
    if let LaicType::List(list_inner) = inner {
        if !is_leaf_type(list_inner) {
            return Err(CompileError::Validation(format!(
                "skill '{skill_name}': field '{field_name}': optional<list<T>> requires T to be a scalar, string, bytes, or tensor type"
            )));
        }
    }
    validate_field_type(skill_name, field_name, inner)
}

fn validate_map_type_constraints(
    skill_name: &str,
    field_name: &str,
    key: &LaicType,
    value: &LaicType,
) -> Result<(), CompileError> {
    match key {
        LaicType::String
        | LaicType::Bool
        | LaicType::I8
        | LaicType::I16
        | LaicType::I32
        | LaicType::I64
        | LaicType::U8 => {}
        _ => {
            return Err(CompileError::Validation(format!(
                "skill '{skill_name}': field '{field_name}': map key must be string, bool, or integer type"
            )));
        }
    }
    match value {
        LaicType::String
        | LaicType::Bytes
        | LaicType::Bool
        | LaicType::I8
        | LaicType::I16
        | LaicType::I32
        | LaicType::I64
        | LaicType::U8
        | LaicType::F32
        | LaicType::F64 => {}
        _ => {
            return Err(CompileError::Validation(format!(
                "skill '{skill_name}': field '{field_name}': map value must be a scalar type"
            )));
        }
    }
    Ok(())
}

fn validate_default_compat(
    skill_name: &str,
    field_name: &str,
    ty: &LaicType,
    default: &Literal,
) -> Result<(), CompileError> {
    match ty {
        LaicType::Tensor { .. }
        | LaicType::List(_)
        | LaicType::Optional(_)
        | LaicType::Bytes
        | LaicType::Map(_, _) => {
            return Err(CompileError::Validation(format!(
                "skill '{skill_name}': field '{field_name}' of type {ty:?} cannot have a default value"
            )));
        }
        _ => {}
    }

    let compatible = matches!(
        (ty, default),
        (LaicType::String, Literal::String(_))
            | (LaicType::Bool, Literal::Bool(_))
            | (
                LaicType::I8
                    | LaicType::I16
                    | LaicType::I32
                    | LaicType::I64
                    | LaicType::U8
                    | LaicType::F32
                    | LaicType::F64,
                Literal::Integer(_)
            )
            | (LaicType::F32 | LaicType::F64, Literal::Float(_))
    );

    if !compatible {
        return Err(CompileError::Validation(format!(
            "skill '{skill_name}': field '{field_name}' has incompatible default value type"
        )));
    }

    if let Literal::Integer(value) = default {
        if let Some((type_name, min, max)) = integer_default_bounds(ty) {
            if *value < min || *value > max {
                return Err(CompileError::Validation(format!(
                    "skill '{skill_name}': field '{field_name}' default value {value} is out of range for {type_name} ({min}..={max})"
                )));
            }
        }
    }

    Ok(())
}

fn validate_errors(
    skill_name: &str,
    errors: &[crate::ast::ErrorVariant],
) -> Result<(), CompileError> {
    let mut names = HashSet::new();
    let mut codes = HashSet::new();

    for variant in errors {
        validate_codegen_identifier(skill_name, "error name", &variant.name)?;
        if !names.insert(&variant.name) {
            return Err(CompileError::Validation(format!(
                "skill '{skill_name}': duplicate error name '{}'",
                variant.name
            )));
        }
        if variant.code == 0 {
            return Err(CompileError::Validation(format!(
                "skill '{skill_name}': error code must be positive, got 0 for '{}'",
                variant.name
            )));
        }
        if !codes.insert(variant.code) {
            return Err(CompileError::Validation(format!(
                "skill '{skill_name}': duplicate error code {} for '{}'",
                variant.code, variant.name
            )));
        }
    }

    Ok(())
}

/// A leaf type is one that codegen can directly map to an Arrow builder/array.
fn is_leaf_type(ty: &LaicType) -> bool {
    matches!(
        ty,
        LaicType::String
            | LaicType::Bytes
            | LaicType::Bool
            | LaicType::I8
            | LaicType::I16
            | LaicType::I32
            | LaicType::I64
            | LaicType::U8
            | LaicType::F32
            | LaicType::F64
            | LaicType::Tensor { .. }
    )
}

fn integer_default_bounds(ty: &LaicType) -> Option<(&'static str, i64, i64)> {
    match ty {
        LaicType::I8 => Some(("i8", i64::from(i8::MIN), i64::from(i8::MAX))),
        LaicType::I16 => Some(("i16", i64::from(i16::MIN), i64::from(i16::MAX))),
        LaicType::I32 => Some(("i32", i64::from(i32::MIN), i64::from(i32::MAX))),
        LaicType::U8 => Some(("u8", i64::from(u8::MIN), i64::from(u8::MAX))),
        _ => None,
    }
}

fn validate_codegen_identifier(
    skill_name: &str,
    context: &str,
    identifier: &str,
) -> Result<(), CompileError> {
    if is_reserved_codegen_identifier(identifier) {
        return Err(CompileError::Validation(format!(
            "skill '{skill_name}': {context} '{identifier}' is a reserved codegen identifier"
        )));
    }
    Ok(())
}

fn is_reserved_codegen_identifier(identifier: &str) -> bool {
    // WHY: LAIC emits one validated identifier into Rust, Python, and TypeScript. Rejecting the
    // cross-target keyword union keeps generated sources parseable without inventing a target-
    // specific mangling scheme that would change the public contract surface per language.
    const RESERVED: &str = "\
        abstract as async await become box break const continue crate do dyn else enum extern \
        false final fn for gen if impl in let loop macro match mod move mut override priv pub ref \
        return Self self static struct super trait true try type typeof union unsafe unsized use \
        virtual where while yield False None True and assert class def del elif except finally from \
        global import is lambda nonlocal not or pass raise with any arguments boolean case catch \
        constructor debugger declare default delete eval export extends function get implements \
        infer instanceof interface keyof module namespace new never null number object of package \
        private protected public readonly require set string switch symbol this throw undefined \
        unique unknown var void";
    RESERVED
        .split_ascii_whitespace()
        .any(|word| word == identifier)
}
