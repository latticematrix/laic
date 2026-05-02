# LAIC Public Export

This repository is the public-safe export target for LAIC.

## Source

- Public target repository: `latticematrix/laic`
- Source provenance is retained in private release records, not in this
  public-facing export document.

## Export Shape

The public export includes release-facing source, package metadata, CI, smoke scripts, crate tests, and release-facing documentation.

It intentionally excludes private development history and local operational material:

- Claude / Codex continuity files
- private planning notes and developer-memory folders
- local performance reports and cloud-test runners
- spikes and exploratory workspaces
- machine-local paths, credentials, and reviewer handoff artifacts

## Release Boundary

This export does not by itself create a release, tag, GitHub Release, crates.io publication, or public repository visibility change.

The current MVP release artifacts remain:

- `laic` Rust crate
- `laicc` Rust crate
- `laicc` CLI

See [RELEASES.md](./RELEASES.md) and [STABILITY.md](./STABILITY.md) for the release gates and stable-surface contract.
