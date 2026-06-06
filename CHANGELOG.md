# Changelog

All notable changes to official LAIC MVP release artifacts are documented in this file.

This changelog tracks release-facing changes to:

- `latrix-laic` package, imported as Rust crate `laic`
- `laicc`
- `laicc` CLI
- the stable-surface contract described in `docs/STABILITY.md`

This changelog does not exist to mirror every internal refactor, repo-local fixture change, or local continuity note.

## Changelog Rules

- Any intentional change to the stable surface must be recorded here.
- Any breaking change must be called out explicitly, with the affected surface named directly.
- Internal-only or experimental-only changes should not be promoted here unless they change release-facing behavior.
- Release notes must stay aligned with `docs/STABILITY.md`.

## 0.2.2 - 2026-06-06

### Release Theme

- Patch release for cross-language contract hardening and QUIC close-lifecycle correctness.

### Changed

- Hardened generated Arrow IPC deserialization so schema type mismatches and tensor metadata mismatches fail closed across Rust, Python, and TypeScript instead of being accepted silently.
- Generated TypeScript `i64` defaults as `bigint` literals.
- Stabilized QUIC transport lifecycle behavior so send/receive after `close()` reports `TransportError::ShuttingDown`.
- Rejected fixed tensor dimension `0` during `.laic` validation so `0` remains the TypeScript dynamic-dimension metadata sentinel and tensor shape semantics stay portable across generated languages.

### Breaking Changes

- `.laic` schemas that use fixed tensor dimension `0` are now rejected. This closes an ambiguous cross-language contract bug: TypeScript generated code uses `0` as the dynamic-dimension metadata sentinel, while Rust and Python treated fixed `0` as a literal shape constraint.

## 0.2.1 - 2026-05-19

### Release Theme

- Patch release for `laicc` CLI diagnostics and onboarding clarity.

### Changed

- Improved `laicc` CLI error reporting so missing input files include the failed path.
- Rejected unsupported `--lang` values during CLI argument parsing before input file I/O.
- Clarified the repository-local `laicc` quickstart in `README.md`, including supported language targets, default target behavior, output file names, and common error cases.
- Added an installed `laicc` quickstart in `README.md` with a self-contained minimal `.laic` example.

### Breaking Changes

- None. The `laicc` CLI parameter names, supported language values, default language, and generated output naming remain unchanged.

## 0.2.0 - 2026-05-07

### Release Theme

- Published the `0.2.0` public line for performance and usability validation evidence closeout.

### Changed

- Aligned the public source manifest with the publishable `latrix-laic` package name while keeping the Rust library crate name `laic`.
- Updated release smoke and CI package checks to use the `latrix-laic` package selector.

### Added

- Added release CI and smoke coverage for package checks, the `laicc` CLI, Rust / Python / TypeScript generation, Python / TypeScript verification, contract-surface compatibility, and boundary checks.
- Added full repo-local usability gate coverage for tier 0 health checks, PowerShell and Git Bash release smoke, Rust / Python / TypeScript minimal consumer paths, a contract-drift fail-closed negative, and a QUIC/mTLS trust-failure negative.
- Added release-facing documentation that keeps `0.1.0` performance evidence version-marked while describing the `0.2.0` validation-readiness theme.

### Breaking Changes

- None. The crates.io package name remains `latrix-laic`, and Rust import paths remain `laic`.

## 0.1.0 - 2026-05-01

### MVP Release Surface

- `latrix-laic` as the mechanism-layer Rust package, imported as Rust crate `laic`.
- `laicc` as the contract compiler crate and CLI.
- The current stable surface described in `docs/STABILITY.md`.

### Added

- Added release-facing onboarding documentation in `README.md`.
- Added this changelog and documented how stable-surface changes must be tracked.
- Added reproducible `release-smoke` scripts and a dedicated CI release-smoke gate for official artifacts.

### Changed

- Clarified that the MVP stable surface is defined by `docs/STABILITY.md`.
- Clarified the minimal release smoke path and which smoke failures are release-blocking.

### Breaking Changes

- None recorded yet.
