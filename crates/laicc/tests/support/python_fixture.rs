use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn fresh_case_dir(root: PathBuf, case_name: &str) -> PathBuf {
    let dir = root.join(case_name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap_or_else(|e| panic!("cleanup {case_name}: {e}"));
    }
    std::fs::create_dir_all(dir.join("generated"))
        .unwrap_or_else(|e| panic!("mkdir {case_name}: {e}"));
    dir
}

pub(crate) fn generated_module_name(fixture: &str) -> String {
    format!("{fixture}_laic")
}

pub(crate) fn generated_import_prelude(fixture: &str) -> String {
    format!("from generated.{} import *", generated_module_name(fixture))
}

pub(crate) fn python_driver_script(fixture: &str, body: &str) -> String {
    format!("{}\n\n{body}\n", generated_import_prelude(fixture))
}

pub(crate) fn python_command(case_dir: &Path) -> Command {
    let mut command = Command::new("python");
    // WHY: package-style verify imports `generated.<fixture>_laic` from a temp case root.
    // Keeping the interpreter entrypoint and PYTHONPATH wiring in one helper prevents
    // python_verify and contract-surface from silently drifting to different package roots.
    command.env("PYTHONPATH", case_dir);
    command
}

pub(crate) fn write_generated_package(case_dir: &Path, fixture: &str, code: &str) -> String {
    // WHY: Python verify and contract-surface compatibility must exercise the same
    // package-style import layout. If this ever forks, one gate can stay green while the
    // other silently drifts to a different module/package shape.
    let module_name = generated_module_name(fixture);
    let module_path = case_dir.join("generated").join(format!("{module_name}.py"));
    std::fs::write(&module_path, code).unwrap_or_else(|e| panic!("write {fixture}: {e}"));
    std::fs::write(case_dir.join("generated").join("__init__.py"), "")
        .unwrap_or_else(|e| panic!("write __init__ for {fixture}: {e}"));
    module_name
}

pub(crate) fn driver_path(case_dir: &Path) -> PathBuf {
    case_dir.join("driver.py")
}
