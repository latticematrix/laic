//! LAICC binary entry point.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(missing_docs)]
#![warn(clippy::pedantic)]

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// LAIC IDL compiler — compiles `.laic` skill contracts to Rust, Python, and TypeScript bindings.
#[derive(Parser)]
#[command(name = "laicc", version, about)]
struct Cli {
    /// Input `.laic` file.
    input: PathBuf,

    /// Target language (`rust`, `python`, or `typescript`).
    #[arg(long, value_enum, default_value = "rust")]
    lang: TargetLanguage,

    /// Output directory.
    #[arg(short, long, default_value = ".")]
    output: PathBuf,
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

    let source = std::fs::read_to_string(&cli.input)
        .map_err(|err| format!("failed to read input file '{}': {err}", cli.input.display()))?;
    let file = laicc::compile(&source).map_err(|err| err.to_string())?;

    let (code, ext) = match cli.lang {
        TargetLanguage::Rust => (laicc::generate_rust(&file), "rs"),
        TargetLanguage::Python => (laicc::generate_python(&file), "py"),
        TargetLanguage::TypeScript => (laicc::generate_typescript(&file), "ts"),
    };
    let code = code.map_err(|err| err.to_string())?;

    let stem = cli
        .input
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
