//! Build script: compile each `.laic` fixture into Rust source via laicc.

use std::path::Path;

fn main() {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("laicc")
        .join("tests")
        .join("fixtures");

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir);

    let entries: Vec<_> = std::fs::read_dir(&fixtures_dir)
        .unwrap_or_else(|e| panic!("cannot read fixtures dir {}: {e}", fixtures_dir.display()))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("laic") {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    assert!(!entries.is_empty(), "no .laic fixtures found");

    for laic_path in &entries {
        let source = std::fs::read_to_string(laic_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", laic_path.display()));

        let file = laicc::compile(&source)
            .unwrap_or_else(|e| panic!("compile failed for {}: {e}", laic_path.display()));

        let code = laicc::generate_rust(&file)
            .unwrap_or_else(|e| panic!("codegen failed for {}: {e}", laic_path.display()));

        let stem = laic_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");

        let rs_path = out_path.join(format!("{stem}_laic.rs"));
        std::fs::write(&rs_path, &code)
            .unwrap_or_else(|e| panic!("cannot write {}: {e}", rs_path.display()));

        println!("cargo:rerun-if-changed={}", laic_path.display());
    }
}
