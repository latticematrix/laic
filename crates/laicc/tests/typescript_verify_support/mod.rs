#[path = "../support/typescript_fixture.rs"]
mod typescript_fixture;

use std::path::PathBuf;

pub(crate) use self::typescript_fixture::{
    cleanup_case_dir, npm_program, runtime_dir, write_package_root_case,
};

pub(crate) fn write_compile_case(case_name: &str, generated_source: &str) -> PathBuf {
    // WHY: `typescript_verify` owns the `.verify` namespace, but it must not fork the
    // package-root layout rules away from `contract_surface_compat`.
    typescript_fixture::write_package_root_case(".verify", case_name, generated_source, None)
}

pub(crate) fn write_roundtrip_case(
    case_name: &str,
    generated_source: &str,
    driver_body: &str,
) -> PathBuf {
    typescript_fixture::write_package_root_case(
        ".verify",
        case_name,
        generated_source,
        Some(driver_body),
    )
}
