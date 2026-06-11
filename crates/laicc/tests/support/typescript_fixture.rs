use std::path::{Path, PathBuf};

const PACKAGE_ROOT_INDEX_TS: &str = include_str!("../typescript_runtime/src/index.ts");

pub(crate) fn runtime_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("typescript_runtime")
}

pub(crate) fn npm_program() -> &'static str {
    // Local Windows dev uses `npm.cmd`, but CI runs on Ubuntu where the executable is
    // plain `npm`. Keep a single decision point so all TS fixture gates follow the same
    // cross-platform invocation rule.
    if cfg!(windows) {
        "npm.cmd"
    } else {
        "npm"
    }
}

pub(crate) fn write_package_root_case(
    scope_dir: &str,
    case_name: &str,
    generated_source: &str,
    driver_body: Option<&str>,
) -> PathBuf {
    let dir = fresh_case_dir(scope_dir, case_name);

    // WHY: both `typescript_verify` and `contract_surface_compat` are supposed to prove
    // the same repo-local package-root shape. Centralizing the layout here keeps `index.ts`
    // and `tsconfig.json` from drifting into two incompatible truths.
    write_case_file(
        &dir.join("src").join("generated.ts"),
        case_name,
        "generated.ts",
        generated_source,
    );
    write_case_file(
        &dir.join("src").join("index.ts"),
        case_name,
        "index.ts",
        PACKAGE_ROOT_INDEX_TS,
    );

    if let Some(driver_body) = driver_body {
        write_case_file(
            &dir.join("src").join("driver.ts"),
            case_name,
            "driver.ts",
            driver_body,
        );
    }

    write_case_file(
        &dir.join("tsconfig.json"),
        case_name,
        "tsconfig.json",
        case_tsconfig(driver_body.is_some()),
    );

    dir
}

pub(crate) fn cleanup_case_dir(path: &Path) {
    if path.exists() {
        std::fs::remove_dir_all(path).unwrap_or_else(|e| panic!("cleanup {}: {e}", path.display()));
    }
    // WHY: the parent `.compat` scope is shared by default-parallel Rust tests. A fixture may
    // remove only its unique case directory; deleting the shared parent races with sibling tests.
}

fn fresh_case_dir(scope_dir: &str, case_name: &str) -> PathBuf {
    let dir = runtime_dir().join(scope_dir).join(case_name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap_or_else(|e| panic!("cleanup {case_name}: {e}"));
    }
    std::fs::create_dir_all(dir.join("src")).unwrap_or_else(|e| panic!("mkdir {case_name}: {e}"));
    dir
}

fn write_case_file(path: &Path, case_name: &str, file_name: &str, contents: &str) {
    std::fs::write(path, contents)
        .unwrap_or_else(|e| panic!("write {file_name} for {case_name}: {e}"));
}

fn case_tsconfig(include_driver: bool) -> &'static str {
    if include_driver {
        r#"{
  "extends": "../../tsconfig.json",
  "compilerOptions": {
    "rootDir": "./src",
    "outDir": "./dist"
  },
  "include": ["./src/generated.ts", "./src/index.ts", "./src/driver.ts"]
}
"#
    } else {
        r#"{
  "extends": "../../tsconfig.json",
  "compilerOptions": {
    "rootDir": "./src",
    "outDir": "./dist"
  },
  "include": ["./src/generated.ts", "./src/index.ts"]
}
"#
    }
}
