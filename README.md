# LAIC

LAIC is an independent mechanism-layer protocol project for high-throughput AI system communication.
It focuses on transport, contract compilation, flow control, transport security, and emergency delivery.
It does not define runtime policy, discovery, routing, provider hosting, or client convenience layers.

## Official MVP Release Artifacts

The current MVP line treats the following as official release artifacts:

- `laic` Rust crate
- `laicc` Rust crate
- `laicc` CLI

The following are not official release artifacts:

- `laicc-verify`
- tests, fixtures, CI helpers, and benchmark harnesses
- local development, planning, review, or continuity materials

The authoritative stability contract for the MVP line lives in [docs/STABILITY.md](./docs/STABILITY.md).

## Performance Evidence (0.1.0 MVP)

The current `0.1.0` MVP evidence shows LAIC's mechanism layer can carry AI-system messages with low overhead across local IPC, localhost QUIC, same-LAN QUIC, and public-WAN QUIC/mTLS test shapes.

Current measured highlights:

- Windows cross-process IPC p95: `43.900us` for 64 KiB payloads.
- Windows localhost QUIC p95: `792.300us` for 64 KiB payloads.
- Same-LAN QUIC p95: `580.900us` fixed-count, `2076.400us` in a 300s soak, and `1246.700us` in a 4-client fan-out.
- Public-WAN QUIC/mTLS to a cloud endpoint stays below `20ms` p95 across the validated two-host fixed-count, 300s soak, and 4-client fan-out shapes.

These are bounded `0.1.0` MVP performance evidence lines, not production SLA claims. See [docs/PERFORMANCE.md](./docs/PERFORMANCE.md) for the full parameter table, version marker, and evidence boundaries.

## What LAIC Does Not Promise

The current MVP does not promise:

- runtime SDKs
- discovery or routing
- provider hosting
- session policy
- retry or reconnect convenience layers
- wider handshake/session/capability semantics beyond the current minimal trust-domain boundary

## Installation

### From This Repository

Use these commands when consuming the repository before a tagged release is published:

```powershell
cargo build -p laic
cargo build -p laicc
cargo install --path crates/laicc
```

### From a Published Release

When the MVP line is published as release artifacts, the official package paths are:

```powershell
cargo add laic
cargo add laicc
cargo install laicc
```

## Quickstart

This minimal smoke path proves the contract/codegen surface.
It does not prove any runtime SDK or provider-hosting capability.

```powershell
cargo run -p laicc -- crates/laicc/tests/fixtures/echo.laic --lang rust -o .tmp/laicc-smoke
```

Expected output:

- generated file: `.tmp/laicc-smoke/echo_laic.rs`

You can switch the target language without changing the contract:

```powershell
cargo run -p laicc -- crates/laicc/tests/fixtures/echo.laic --lang python -o .tmp/laicc-smoke
cargo run -p laicc -- crates/laicc/tests/fixtures/echo.laic --lang typescript -o .tmp/laicc-smoke
```

## Release Smoke

Run one of these from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\release-smoke.ps1
bash ./scripts/release-smoke.sh
```

This smoke proves only that:

- official artifacts can be packaged
- the `laicc` CLI can be invoked
- the minimal Rust / Python / TypeScript generation path succeeds

This does not prove runtime, discovery, routing, provider hosting, or client SDK behavior.

For the current MVP line, the following failures are release-blocking:

- `cargo package -p laic --allow-dirty`
- `cargo package -p laicc --allow-dirty`
- `cargo run -p laicc -- --help`
- any missing Rust / Python / TypeScript output expected by `scripts/release-smoke.*`

## Release-Facing Docs

Start with these files:

- [README.md](./README.md) for onboarding and the minimal quickstart
- [docs/BOUNDARY.md](./docs/BOUNDARY.md) for the mechanism-vs-policy boundary
- [docs/STABILITY.md](./docs/STABILITY.md) for stable vs internal/experimental surface
- [docs/PERFORMANCE.md](./docs/PERFORMANCE.md) for measured performance evidence and boundaries
- [docs/RELEASES.md](./docs/RELEASES.md) for release readiness gates
- [docs/PUBLIC_EXPORT.md](./docs/PUBLIC_EXPORT.md) for public-export provenance
- [CHANGELOG.md](./CHANGELOG.md) for stable-surface changes

## License

LAIC is licensed under the Apache License, Version 2.0. See [LICENSE](./LICENSE).

## Scope Guard

The public boundary authority is [docs/BOUNDARY.md](./docs/BOUNDARY.md).
The practical summary is simple:

- LAIC is mechanism, not runtime policy
- the stable surface is intentionally smaller than the repository's total public visibility
- anything not listed in [docs/STABILITY.md](./docs/STABILITY.md) should be treated as internal or experimental
