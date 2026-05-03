# Changelog

All notable changes to official LAIC MVP release artifacts are documented in this file.

This changelog tracks release-facing changes to:

- `latrix-laic` (library crate name: `laic`)
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

### Changed

- Clarified that the MVP stable surface is defined by `docs/STABILITY.md`.
- Clarified the minimal release smoke path and which smoke failures are release-blocking.
- Renamed the published Rust package from `laic` to `latrix-laic` while keeping the library crate name as `laic`.

## 0.1.0 - 2026-05-01

### MVP Release Surface

- `laic` as the mechanism-layer Rust crate.
- `laicc` as the contract compiler crate and CLI.
- The current stable surface described in `docs/STABILITY.md`.

### Added

- Added release-facing onboarding documentation in `README.md`.
- Added this changelog and documented how stable-surface changes must be tracked.
- Added reproducible `release-smoke` scripts and a dedicated CI release-smoke gate for official artifacts.

### Breaking Changes

- None recorded yet.
