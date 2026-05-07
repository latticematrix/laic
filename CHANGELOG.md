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

## Unreleased

The `0.2.0` release candidate is not yet published.

## 0.2.0 - Pending Release

### Release Theme

- Prepares the public release-candidate line for performance and usability validation evidence closeout.

### Changed

- Aligned the public source manifest with the publishable `latrix-laic` package name while keeping the Rust library crate name `laic`.
- Updated release smoke and CI package checks to use the `latrix-laic` package selector.

### Added

- Added release-candidate CI and smoke coverage for package checks, the `laicc` CLI, Rust / Python / TypeScript generation, Python / TypeScript verification, contract-surface compatibility, and boundary checks.
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
