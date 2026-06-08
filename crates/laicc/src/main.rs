//! LAICC binary entry point.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(missing_docs)]
#![warn(clippy::pedantic)]

use clap::{Parser, Subcommand, ValueEnum};
use laicc::{Dimension, FieldDef, LaicFile, LaicType, SkillDef, StructDef, TensorElementType};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// LAIC IDL compiler — compiles `.laic` skill contracts to Rust, Python, and TypeScript bindings.
#[derive(Parser)]
#[command(name = "laicc", version, about)]
struct Cli {
    /// Input `.laic` file.
    input: Option<PathBuf>,

    /// Command to run.
    #[command(subcommand)]
    command: Option<Command>,

    /// Target language (`rust`, `python`, or `typescript`).
    #[arg(long, value_enum, default_value = "rust")]
    lang: TargetLanguage,

    /// Output directory.
    #[arg(short, long, default_value = ".")]
    output: PathBuf,
}

#[derive(Subcommand)]
enum Command {
    /// Print a human-readable Arrow schema and LAIC metadata diagnostic.
    InspectSchema {
        /// Input `.laic` file.
        input: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum TargetLanguage {
    Rust,
    Python,
    #[value(name = "typescript")]
    TypeScript,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("laicc: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::InspectSchema { input }) => {
            let file = read_contract(&input)?;
            println!("{}", inspect_schema(&input, &file));
            Ok(())
        }
        None => generate_bindings(cli),
    }
}

fn generate_bindings(cli: Cli) -> Result<(), String> {
    let input = cli
        .input
        .ok_or_else(|| "missing input `.laic` file".to_string())?;
    let file = read_contract(&input)?;

    let (code, ext) = match cli.lang {
        TargetLanguage::Rust => (laicc::generate_rust(&file), "rs"),
        TargetLanguage::Python => (laicc::generate_python(&file), "py"),
        TargetLanguage::TypeScript => (laicc::generate_typescript(&file), "ts"),
    };
    let code = code.map_err(|err| err.to_string())?;

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let out_path = cli.output.join(format!("{stem}_laic.{ext}"));
    std::fs::create_dir_all(&cli.output).map_err(|err| {
        format!(
            "failed to create output directory '{}': {err}",
            cli.output.display()
        )
    })?;
    std::fs::write(&out_path, code).map_err(|err| {
        format!(
            "failed to write output file '{}': {err}",
            out_path.display()
        )
    })?;
    eprintln!("wrote {}", out_path.display());

    Ok(())
}

fn read_contract(input: &Path) -> Result<LaicFile, String> {
    let source = std::fs::read_to_string(input)
        .map_err(|err| format!("failed to read input file '{}': {err}", input.display()))?;
    laicc::compile(&source).map_err(|err| err.to_string())
}

fn inspect_schema(input: &Path, file: &LaicFile) -> String {
    let mut out = String::new();
    out.push_str("LAIC schema inspection\n");
    // WHY: first-slice inspect-schema is diagnostic text for humans. A stable
    // JSON/YAML contract would need a separate authority and compatibility gate.
    out.push_str("Format: human-readable diagnostic only; exact wording is not stable.\n");
    let _ = writeln!(out, "Contract: {}", input.display());
    let _ = writeln!(out, "Version: {}", file.version);

    for skill in &file.skills {
        inspect_skill(&mut out, skill, &file.version);
    }

    out
}

fn inspect_skill(out: &mut String, skill: &SkillDef, version: &str) {
    let _ = writeln!(out, "\nSkill: {}", skill.name);
    out.push_str("  Schema metadata:\n");
    let _ = writeln!(out, "    laic.skill_id = {}", skill.id);
    let _ = writeln!(out, "    laic.version = {version}");
    inspect_struct(out, &skill.input, "input");
    inspect_struct(out, &skill.output, "output");
}

fn inspect_struct(out: &mut String, def: &StructDef, direction: &str) {
    let _ = writeln!(out, "  Message: {}", def.name);
    let _ = writeln!(out, "    laic.direction = {direction}");
    for field in &def.fields {
        inspect_field(out, field);
    }
}

fn inspect_field(out: &mut String, field: &FieldDef) {
    let _ = writeln!(out, "    Field: {}", field.name);
    let _ = writeln!(out, "      LAIC type: {}", format_laic_type(&field.ty));
    let _ = writeln!(out, "      Arrow type: {}", format_arrow_type(&field.ty));
    let _ = writeln!(out, "      Nullable: {}", is_nullable(&field.ty));

    if let Some((dtype, dims)) = tensor_metadata(&field.ty) {
        out.push_str("      Metadata:\n");
        let _ = writeln!(out, "        laic.tensor.dtype = {}", dtype.as_str());
        let _ = writeln!(
            out,
            "        laic.tensor.shape = {}",
            format_tensor_shape_metadata(dims)
        );
        out.push_str("        laic.tensor.version = 1\n");
    }
}

fn is_nullable(ty: &LaicType) -> bool {
    matches!(ty, LaicType::Optional(_))
}

fn tensor_metadata(ty: &LaicType) -> Option<(&TensorElementType, &[Dimension])> {
    match ty {
        LaicType::Tensor { dtype, dims } => Some((dtype, dims)),
        LaicType::List(inner) | LaicType::Optional(inner) => match inner.as_ref() {
            LaicType::Tensor { dtype, dims } => Some((dtype, dims)),
            _ => None,
        },
        _ => None,
    }
}

fn format_laic_type(ty: &LaicType) -> String {
    match ty {
        LaicType::String => "string".into(),
        LaicType::Bytes => "bytes".into(),
        LaicType::Bool => "bool".into(),
        LaicType::I8 => "i8".into(),
        LaicType::I16 => "i16".into(),
        LaicType::I32 => "i32".into(),
        LaicType::I64 => "i64".into(),
        LaicType::U8 => "u8".into(),
        LaicType::F32 => "f32".into(),
        LaicType::F64 => "f64".into(),
        LaicType::Tensor { dtype, dims } => {
            format!("tensor<{}>{}", dtype.as_str(), format_laic_dims(dims))
        }
        LaicType::List(inner) => format!("list<{}>", format_laic_type(inner)),
        LaicType::Optional(inner) => format!("optional<{}>", format_laic_type(inner)),
        LaicType::Map(key, value) => {
            format!(
                "map<{}, {}>",
                format_laic_type(key),
                format_laic_type(value)
            )
        }
    }
}

fn format_laic_dims(dims: &[Dimension]) -> String {
    let parts = dims
        .iter()
        .map(|dim| match dim {
            Dimension::Fixed(size) => size.to_string(),
            Dimension::Dynamic(Some(name)) => name.clone(),
            Dimension::Dynamic(None) => "_".into(),
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{parts}]")
}

fn format_tensor_shape_metadata(dims: &[Dimension]) -> String {
    let parts = dims
        .iter()
        .map(|dim| match dim {
            Dimension::Fixed(size) => size.to_string(),
            Dimension::Dynamic(_) => "0".into(),
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{parts}]")
}

fn format_arrow_type(ty: &LaicType) -> String {
    match ty {
        LaicType::String => "DataType::Utf8".into(),
        LaicType::Bytes | LaicType::Tensor { .. } => "DataType::Binary".into(),
        LaicType::Bool => "DataType::Boolean".into(),
        LaicType::I8 => "DataType::Int8".into(),
        LaicType::I16 => "DataType::Int16".into(),
        LaicType::I32 => "DataType::Int32".into(),
        LaicType::I64 => "DataType::Int64".into(),
        LaicType::U8 => "DataType::UInt8".into(),
        LaicType::F32 => "DataType::Float32".into(),
        LaicType::F64 => "DataType::Float64".into(),
        LaicType::List(inner) => format!("DataType::List(item: {})", format_arrow_type(inner)),
        LaicType::Optional(inner) => format_arrow_type(inner),
        LaicType::Map(key, value) => format!(
            "DataType::Map(keys: {}, values: {})",
            format_arrow_type(key),
            format_arrow_type(value)
        ),
    }
}
