# LAIC Releases

This document describes how LAIC release readiness is evaluated for public
distribution. It does not announce that a release, publication, or crates.io
upload has already happened.

## Release Artifacts

The current MVP release artifacts are:

- `latrix-laic` Rust package, imported as Rust crate `laic`;
- `laicc` Rust crate;
- `laicc` CLI.

The following are not release artifacts:

- `laicc-verify`;
- tests, fixtures, benches, and local verification harnesses;
- local continuity records;
- internal planning or review materials.

## Release Gates

A release candidate must pass these gates before publication is considered:

- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo test --workspace`;
- release smoke on a supported shell;
- package listing checks for `latrix-laic` and `laicc`, including packaged `LICENSE`
  files in both official crate source packages;
- stability-surface review against `docs/STABILITY.md`;
- boundary review against `docs/BOUNDARY.md`;
- changelog review;
- license metadata and repository license file review;
- repository metadata review;
- public-link review;
- secret and internal-marker scan on the public repository contents.

Passing these gates means the candidate is ready for release review. It does
not by itself create a tag, GitHub Release, crates.io publication, or public
repository visibility change.

## Release Smoke

Release smoke verifies that:

- official packages can be packaged;
- the `laicc` CLI can be invoked;
- minimal Rust, Python, and TypeScript contract generation succeeds.

Release smoke does not verify runtime SDK behavior, provider hosting, discovery,
routing, or application policy.

## Source Package Contents

Cargo source packages may include crate-local tests, fixtures, and benches when they are part of the crate source package. These files are not separate runtime dependencies and are not treated as official release artifacts. They exist so downstream users and reviewers can inspect or re-run crate-local verification from the source package.

Do not treat the presence of `tests/`, `fixtures/`, or `benches/` in `cargo package --list` as LAIC promising a test SDK, benchmark product, runtime harness, or public validation service.

## Versioning

Before `1.0`, LAIC still treats the stable surface in `docs/STABILITY.md` as
protected. Breaking stable-surface changes require explicit documentation and
review even when the version number is below `1.0`.

After `1.0`, stable-surface breaks require a major-version boundary.

## Public Repository Cutover

The public repository is a clean downstream target. It receives only public-safe
content exported from the controlled development upstream.

Public repository preparation is separate from release publication:

- exporting public-safe content does not publish a release;
- passing CI does not publish a release;
- creating a tag does not publish crates;
- publishing crates requires a separate explicit approval step.

Each public export should record the source commit in controlled release records
and the public commit or tag that was produced from it.
