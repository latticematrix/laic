# Contributing To LAIC

Thank you for considering a contribution to LAIC.

LAIC is a mechanism-layer protocol project. Contributions are welcome when they
improve LAIC-owned transport, contract, code generation, flow-control, emergency
delivery, release, documentation, or verification surfaces without crossing the
project boundary.

## Scope

Good contribution candidates include:

- `laic` transport, protocol, flow-control, emergency-channel, and minimal
  trust-domain handshake fixes;
- `.laic` contract language and `laicc` compiler fixes;
- Rust, Python, or TypeScript generated-surface correctness fixes;
- release-smoke, package, CI, and documentation improvements;
- mechanism-only examples that can run from a clean checkout;
- security reporting, release, and contribution process improvements.

Out-of-scope contributions include:

- Runtime, Core, or Secure integration promises;
- discovery, routing, scheduling, orchestration, or model-selection behavior;
- provider hosting, marketplace, workflow, or multi-agent application demos;
- public TCK, certification, or compatibility claims that are not separately
  approved;
- performance comparison claims without reproducible benchmark evidence.

When in doubt, start from [docs/BOUNDARY.md](./docs/BOUNDARY.md) and
[docs/STABILITY.md](./docs/STABILITY.md). The stable surface is smaller than the
repository's visible implementation and test layout.

## Before Opening A Change

For behavior, compatibility, release, or security-sensitive work, open an issue
or short design note first. Small documentation fixes, typo fixes, or narrow
test improvements can go straight to a pull request.

Before editing, classify the change:

- `no-release`: internal cleanup or documentation with no public adoption value;
- `0.2.x adoption/patch`: public onboarding, contribution, security,
  packaging, release-process, or user-facing patch value without behavior
  change;
- `0.3.0`: new protocol behavior, stable-surface expansion, or material
  transport semantics;
- `reject`: work outside the LAIC mechanism-layer boundary.

Do not use a `0.2.x` patch as a way to smuggle in protocol behavior, stable
surface, Runtime/Core/Secure, routing, provider, workflow, marketplace, or
multi-agent scope.

## Pull Request Checklist

For most changes, run:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

For release-facing or packaging changes, also run the release smoke path that
matches your shell:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\release-smoke.ps1
```

```bash
bash ./scripts/release-smoke.sh
```

In the pull request, include:

- the goal and non-goal;
- changed files or affected surface;
- validation commands and results;
- whether the change affects `docs/STABILITY.md`, `CHANGELOG.md`, or
  `docs/RELEASES.md`;
- boundary notes if the topic is near Runtime/Core/Secure, A2A, routing,
  provider hosting, workflow, marketplace, or multi-agent terminology.

## Stable Surface Changes

Changes to the current MVP stable surface must update the relevant release-facing
docs. At minimum, review:

- [docs/STABILITY.md](./docs/STABILITY.md);
- [CHANGELOG.md](./CHANGELOG.md);
- [docs/RELEASES.md](./docs/RELEASES.md).

Patch releases must not contain silent breaking changes. New protocol behavior,
new stable APIs, or material transport semantics need a specific `0.3.0` plan
before implementation.

## Security Issues

Do not open a public issue with exploit details, credentials, private keys, or
reproduction material that could harm users. Follow [SECURITY.md](./SECURITY.md)
instead.
