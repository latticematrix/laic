//! LAICC binary entry point.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(missing_docs)]
#![warn(clippy::pedantic)]

use clap::Parser;
use std::path::PathBuf;

/// LAIC IDL compiler — compiles `.laic` skill contracts to Rust, Python, and TypeScript bindings.
#[derive(Parser)]
#[command(name = "laicc", version, about)]
struct Cli {
    /// Input `.laic` file.
    input: PathBuf,

    /// Target language (`rust`, `python`, or `typescript`).
    #[arg(long, default_value = "rust")]
    lang: String,

    /// Output directory.
    #[arg(short, long, default_value = ".")]
    output: PathBuf,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("laicc: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), laicc::CompileError> {
    let cli = Cli::parse();

    let source = std::fs::read_to_string(&cli.input)?;
    let file = laicc::compile(&source)?;

    let (code, ext) = match cli.lang.as_str() {
        "rust" => (laicc::generate_rust(&file)?, "rs"),
        "python" => (laicc::generate_python(&file)?, "py"),
        "typescript" => (laicc::generate_typescript(&file)?, "ts"),
        other => {
            return Err(laicc::CompileError::Codegen(format!(
                "unsupported target language: '{other}' (available: rust, python, typescript)"
            )));
        }
    };

    let stem = cli
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let out_path = cli.output.join(format!("{stem}_laic.{ext}"));
    std::fs::create_dir_all(&cli.output)?;
    std::fs::write(&out_path, code)?;
    eprintln!("wrote {}", out_path.display());

    Ok(())
}
