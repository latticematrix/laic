#![cfg(feature = "contract-surface-verify")]

#[path = "support/contract_surface.rs"]
mod contract_surface;

use contract_surface::prepare_fixture;

fn assert_contract_surface_compat(fixture: &str) {
    let harness = prepare_fixture(fixture);
    let canonical = harness.canonical_snapshot();
    let python = harness.python_snapshot();
    let typescript = harness.typescript_snapshot();
    let expected_cross_language = harness.expected_cross_language_observation();

    assert_eq!(
        python, canonical,
        "python snapshot drifted from Rust canonical contract for {fixture}"
    );
    assert_eq!(
        typescript, canonical,
        "typescript snapshot drifted from Rust canonical contract for {fixture}"
    );
    assert_eq!(
        harness.roundtrip_python_to_typescript(),
        expected_cross_language,
        "python -> typescript IPC roundtrip drifted for {fixture}"
    );
    assert_eq!(
        harness.roundtrip_typescript_to_python(),
        expected_cross_language,
        "typescript -> python IPC roundtrip drifted for {fixture}"
    );
}

#[test]
fn echo_contract_surface_compatibility() {
    assert_contract_surface_compat("echo.laic");
}

#[test]
fn escaped_defaults_contract_surface_compatibility() {
    assert_contract_surface_compat("escaped_defaults.laic");
}

#[test]
fn embedding_contract_surface_compatibility() {
    assert_contract_surface_compat("embedding.laic");
}

#[test]
fn optional_types_contract_surface_compatibility() {
    assert_contract_surface_compat("optional_types.laic");
}

#[test]
fn contract_surface_fixture_cleans_repo_local_typescript_outputs() {
    let case_dir = {
        let harness = prepare_fixture("echo.laic");
        harness.typescript_case_dir().to_path_buf()
    };

    assert!(
        !case_dir.exists(),
        "contract-surface fixture left repo-local TypeScript outputs behind: {}",
        case_dir.display()
    );
}
