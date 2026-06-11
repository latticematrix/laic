#[path = "contract_surface/drivers.rs"]
mod drivers;
#[path = "contract_surface/fixture.rs"]
mod fixture;
#[path = "python_fixture.rs"]
mod python_fixture;
#[path = "typescript_fixture.rs"]
mod typescript_fixture;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use laicc::LaicFile;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use self::drivers::{python_driver, typescript_driver};
use self::fixture::{fixture_spec, FixtureSpec};
use self::python_fixture::{
    driver_path as python_driver_path, fresh_case_dir as fresh_python_case_dir, python_command,
    write_generated_package as write_python_package,
};
use self::typescript_fixture::{
    cleanup_case_dir, npm_program, runtime_dir, write_package_root_case,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ContractSnapshot {
    pub(crate) input: SurfaceSnapshot,
    pub(crate) output: SurfaceSnapshot,
    pub(crate) errors: BTreeMap<String, u16>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SurfaceSnapshot {
    pub(crate) skill_id: String,
    pub(crate) version: String,
    pub(crate) direction: String,
    pub(crate) fields: BTreeMap<String, Value>,
    pub(crate) schema_metadata: BTreeMap<String, String>,
    pub(crate) row_count: usize,
    pub(crate) record_batch_count: usize,
    pub(crate) rejects_multiple_batches: bool,
    pub(crate) rejects_tensor_dtype_mismatch: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct CrossLanguageObservation {
    pub(crate) input: BTreeMap<String, Value>,
    pub(crate) output: BTreeMap<String, Value>,
}

pub(crate) struct PreparedFixture {
    file: LaicFile,
    spec: FixtureSpec,
    case_scope: PathBuf,
    python_driver: PathBuf,
    python_case_dir: PathBuf,
    runtime_json: String,
    typescript_case_dir: PathBuf,
    typescript_driver: PathBuf,
}

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) fn prepare_fixture(fixture: &str) -> PreparedFixture {
    let spec = fixture_spec(fixture);
    let source = load_fixture_source(spec.stem);
    let file = laicc::compile(&source).unwrap_or_else(|e| panic!("compile {}: {e}", spec.stem));
    let python_code = laicc::generate_python(&file)
        .unwrap_or_else(|e| panic!("generate_python {}: {e}", spec.stem));
    let typescript_code = laicc::generate_typescript(&file)
        .unwrap_or_else(|e| panic!("generate_typescript {}: {e}", spec.stem));
    let runtime_json = serde_json::to_string(&spec.runtime)
        .unwrap_or_else(|e| panic!("serialize {}: {e}", spec.stem));
    let case_id = unique_case_id(spec.stem);
    let case_scope = std::env::temp_dir()
        .join("laicc_contract_surface")
        .join(&case_id);
    let (python_case_dir, python_driver) = write_python_case(&case_scope, &spec, &python_code);
    let (typescript_case_dir, typescript_driver) =
        write_typescript_case(&case_id, &typescript_code);
    compile_typescript_case(&typescript_case_dir, spec.stem);

    PreparedFixture {
        file,
        spec,
        case_scope,
        python_driver,
        python_case_dir,
        runtime_json,
        typescript_case_dir,
        typescript_driver,
    }
}

impl PreparedFixture {
    pub(crate) fn typescript_case_dir(&self) -> &Path {
        &self.typescript_case_dir
    }

    pub(crate) fn canonical_snapshot(&self) -> ContractSnapshot {
        let skill = &self.file.skills[0];
        ContractSnapshot {
            input: canonical_surface(
                &skill.id,
                &self.file.version,
                "input",
                self.spec.expected_input.clone(),
                false,
            ),
            output: canonical_surface(
                &skill.id,
                &self.file.version,
                "output",
                self.spec.expected_output.clone(),
                self.spec.runtime.tensor_field.is_some(),
            ),
            errors: skill
                .errors
                .iter()
                .map(|variant| (variant.name.clone(), variant.code))
                .collect(),
        }
    }

    pub(crate) fn expected_cross_language_observation(&self) -> CrossLanguageObservation {
        CrossLanguageObservation {
            input: self.spec.expected_input.clone(),
            output: self.spec.expected_output.clone(),
        }
    }

    pub(crate) fn python_snapshot(&self) -> ContractSnapshot {
        parse_json_output(
            run_python(
                &self.python_case_dir,
                &self.python_driver,
                &self.runtime_json,
                "snapshot",
                &[],
                self.spec.stem,
            ),
            &format!("python snapshot {}", self.spec.stem),
        )
    }

    pub(crate) fn typescript_snapshot(&self) -> ContractSnapshot {
        parse_json_output(
            run_node(
                &self.typescript_case_dir,
                &self.typescript_driver,
                &self.runtime_json,
                "snapshot",
                &[],
                self.spec.stem,
            ),
            &format!("typescript snapshot {}", self.spec.stem),
        )
    }

    pub(crate) fn roundtrip_python_to_typescript(&self) -> CrossLanguageObservation {
        let exchange_dir = fresh_exchange_dir(&self.case_scope, "python_to_typescript");
        assert_process_succeeded(
            &run_python(
                &self.python_case_dir,
                &self.python_driver,
                &self.runtime_json,
                "produce",
                &[&exchange_dir],
                self.spec.stem,
            ),
            &format!("python producer {}", self.spec.stem),
        );
        parse_json_output(
            run_node(
                &self.typescript_case_dir,
                &self.typescript_driver,
                &self.runtime_json,
                "consume",
                &[
                    &exchange_dir.join("input.ipc"),
                    &exchange_dir.join("output.ipc"),
                ],
                self.spec.stem,
            ),
            &format!("typescript consumer {}", self.spec.stem),
        )
    }

    pub(crate) fn roundtrip_typescript_to_python(&self) -> CrossLanguageObservation {
        let exchange_dir = fresh_exchange_dir(&self.case_scope, "typescript_to_python");
        assert_process_succeeded(
            &run_node(
                &self.typescript_case_dir,
                &self.typescript_driver,
                &self.runtime_json,
                "produce",
                &[&exchange_dir],
                self.spec.stem,
            ),
            &format!("typescript producer {}", self.spec.stem),
        );
        parse_json_output(
            run_python(
                &self.python_case_dir,
                &self.python_driver,
                &self.runtime_json,
                "consume",
                &[
                    &exchange_dir.join("input.ipc"),
                    &exchange_dir.join("output.ipc"),
                ],
                self.spec.stem,
            ),
            &format!("python consumer {}", self.spec.stem),
        )
    }
}

impl Drop for PreparedFixture {
    fn drop(&mut self) {
        // WHY: this harness writes TypeScript fixture packages under the repo-local runtime tree so
        // `tsc` can inherit the checked-in fixture config. Cleanup keeps test runs from leaving
        // untracked `.compat` outputs behind, which would otherwise pollute review/audit state.
        remove_dir_if_exists(&self.python_case_dir);
        remove_dir_if_exists(&self.case_scope);
        cleanup_case_dir(&self.typescript_case_dir);
    }
}

fn canonical_surface(
    skill_id: &str,
    version: &str,
    direction: &str,
    fields: BTreeMap<String, Value>,
    rejects_tensor_dtype_mismatch: bool,
) -> SurfaceSnapshot {
    SurfaceSnapshot {
        skill_id: skill_id.to_string(),
        version: version.to_string(),
        direction: direction.to_string(),
        fields,
        schema_metadata: BTreeMap::from([
            ("laic.direction".to_string(), direction.to_string()),
            ("laic.skill_id".to_string(), skill_id.to_string()),
            ("laic.version".to_string(), version.to_string()),
        ]),
        row_count: 1,
        record_batch_count: 1,
        rejects_multiple_batches: true,
        rejects_tensor_dtype_mismatch,
    }
}

fn parse_json_output<T: for<'de> Deserialize<'de>>(output: Output, context: &str) -> T {
    assert_process_succeeded(&output, context);
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "invalid JSON for {context}: {e}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn load_fixture_source(stem: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(format!("{stem}.laic"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn write_python_case(case_scope: &Path, spec: &FixtureSpec, generated: &str) -> (PathBuf, PathBuf) {
    let dir = fresh_python_case_dir(case_scope.to_path_buf(), "python");
    write_python_package(&dir, spec.stem, generated);
    let driver = python_driver_path(&dir);
    fs::write(&driver, python_driver(spec.stem))
        .unwrap_or_else(|e| panic!("write python driver {}: {e}", spec.stem));
    (dir, driver)
}

fn write_typescript_case(case_id: &str, generated: &str) -> (PathBuf, PathBuf) {
    let dir = write_package_root_case(".compat", case_id, generated, Some(typescript_driver()));
    (dir.clone(), dir.join("dist").join("driver.js"))
}

fn compile_typescript_case(case_dir: &Path, stem: &str) {
    let output = Command::new(npm_program())
        .args([
            "exec",
            "tsc",
            "--",
            "--project",
            case_dir.join("tsconfig.json").to_str().unwrap_or(""),
        ])
        .current_dir(runtime_dir())
        .output()
        .unwrap_or_else(|e| panic!("tsc not found for {stem}: {e}"));
    assert_process_succeeded(&output, &format!("typescript compile {stem}"));
}

fn fresh_dir(path: PathBuf) -> PathBuf {
    if path.exists() {
        fs::remove_dir_all(&path).unwrap_or_else(|e| panic!("cleanup {}: {e}", path.display()));
    }
    path
}

fn remove_dir_if_exists(path: &Path) {
    if path.exists() {
        fs::remove_dir_all(path).unwrap_or_else(|e| panic!("cleanup {}: {e}", path.display()));
    }
}

fn fresh_exchange_dir(case_scope: &Path, name: &str) -> PathBuf {
    let dir = fresh_dir(case_scope.join(name));
    fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("mkdir exchange {}: {e}", dir.display()));
    dir
}

fn unique_case_id(stem: &str) -> String {
    // WHY: Rust runs test functions in parallel by default. Several contract-surface tests exercise
    // the same fixture stem (`echo`) and their Drop cleanup can otherwise delete another test's
    // Python driver or IPC exchange directory mid-run.
    let sequence = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
    format!("{stem}-{}-{sequence}", std::process::id())
}

fn run_python(
    case_dir: &Path,
    driver: &Path,
    runtime_json: &str,
    mode: &str,
    paths: &[&Path],
    stem: &str,
) -> Output {
    let mut command = python_command(case_dir);
    command
        .arg(driver)
        .arg(mode)
        .env("LAIC_COMPAT_CONFIG", runtime_json);
    for path in paths {
        command.arg(path);
    }
    command
        .output()
        .unwrap_or_else(|e| panic!("python not found for {stem}: {e}"))
}

fn run_node(
    case_dir: &Path,
    driver: &Path,
    runtime_json: &str,
    mode: &str,
    paths: &[&Path],
    stem: &str,
) -> Output {
    let mut command = Command::new("node");
    command
        .arg(driver)
        .arg(mode)
        .env("LAIC_COMPAT_CONFIG", runtime_json);
    for path in paths {
        command.arg(path);
    }
    command
        .current_dir(case_dir)
        .output()
        .unwrap_or_else(|e| panic!("node not found for {stem}: {e}"))
}

fn assert_process_succeeded(output: &Output, context: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "process failed for {context} with status {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
}
