# LAIC Stability And Release Surface

> This file is the release-facing source of truth for what the current LAIC MVP does and does not promise.
> It operates under the higher-level public scope guard in [BOUNDARY.md](./BOUNDARY.md).
> Start with [README.md](../README.md) for onboarding, [docs/RELEASES.md](./RELEASES.md) for release gates, and [CHANGELOG.md](../CHANGELOG.md) for release-facing change history.

## What Is Officially Released

Current MVP official release artifacts are limited to:

- `laic` Rust crate
- `laicc` Rust crate
- `laicc` CLI

Current MVP explicitly does not treat the following as official release artifacts:

- `laicc-verify`
- repo-local tests, fixtures, CI helpers, and bench harnesses
- local development, planning, review, or continuity materials

LAIC release status remains `laic-only`.
It does not depend on `Latrix` or any downstream integration being released at the same time.
External users should treat `README.md` as the onboarding entry point, this file as the stability contract, and `CHANGELOG.md` as the history of stable-surface changes.

## Stable Surface

Only the surfaces listed in this section are compatibility-protected for the current MVP scope.

### `laic` crate

The stable Rust API for `laic` is the documented crate-root surface centered on these exported protocol mechanisms:

- transport entry points:
  - `Transport`
  - `IpcConnection`
  - `QuicConnection`
  - `QuicServer`
  - `ClientTlsConfig`
  - `ServerTlsConfig`
- protocol/data-plane control types:
  - `MessageHeader`
  - `Message`
  - `MsgType`
  - `PayloadFormat`
  - `Qos`
- error surface:
  - `ErrorCode`
  - `LaicError`
  - `TransportError`
  - `CodecError`
  - `ProtocolError`
  - `FlowError`
- flow/emergency/gateway mechanism surface:
  - `CreditController`
  - `EmergencyChannel`
  - `Gateway`
- minimal trust-domain handshake surface:
  - `client_handshake`
  - `server_handshake`
  - `ClientHandshakeConfig`
  - `ServerHandshakeConfig`
  - `TrustDomainSession`

The stable promise is tied to the documented crate-root API.
Module topology below crate root is not a separate compatibility promise just because an item is currently reachable through a `pub mod` path.

### `laicc` library

The stable `laicc` library contract is the top-level compile/codegen workflow:

- `compile()`
- `generate_rust()`
- `generate_python()`
- `generate_typescript()`

Support types named directly by those signatures are re-exported at the crate root and included only to the extent required to use that top-level workflow:

- `CompileError`
- `LaicFile`
- `SkillDef`
- `StructDef`
- `FieldDef`
- `LaicType`
- `TensorElementType`
- `Dimension`
- `ErrorVariant`
- `Literal`

Submodule layout under `ast`, `parser`, `validate`, and `codegen` is not independently frozen as public architecture.

### `laicc` CLI

The stable CLI contract for the current MVP is:

```text
laicc [--lang rust|python|typescript] [-o <output-dir>] <input>
```

This includes:

- required single input `.laic` file
- optional `--lang` target selection for `rust`, `python`, and `typescript`, with current default `rust`
- optional `-o` / `--output` directory selection, with current default `.`
- generated file naming convention: `<stem>_laic.rs|py|ts`

It does not currently promise config-file loading, preset/profile systems, runtime scaffolding, package publishing helpers, or project-generation workflows.

### `.laic` contract and generated contract rules

The current MVP stable contract surface includes:

- skill metadata identity and direction semantics
- default-value semantics across Rust / Python / TypeScript generation
- cross-language error-code matching where the contract surface already locks numeric meaning
- Arrow IPC schema metadata and single-record / single-`RecordBatch` cardinality rules
- tensor metadata rejection rules already ratified as contract-surface invariants

### Error-code and handshake compatibility

The current MVP stable protocol-compatibility surface includes:

- published `ErrorCode` numeric meaning for the current protocol/codec/transport/flow ranges
- already-ratified handshake protocol codes, including:
  - `UnsupportedHandshakeVersion = 0x0308`
  - `TrustDomainMismatch = 0x0309`
  - `HandshakeNonceMismatch = 0x030A`
  - `InvalidHandshakePayload = 0x030B`
  - `UnexpectedPayloadFormat = 0x030C`
- the minimal trust-domain handshake boundary already documented by current authority:
  - minimal field set
  - `HelloAck` success/rejection shape split
  - `client_nonce` echo requirement
  - A2 is not part of the current stable protocol promise

## Internal / Experimental Surface

The following are intentionally outside the current MVP stable promise:

- `laicc-verify`
- direct use of `laicc::parser`, `laicc::validate`, `laicc::codegen::*`, `laicc::ast`, `laicc::error`, or internal AST/layout details beyond the documented top-level workflow
- `laic` internal module layout, transport implementation details, protobuf/Arrow helper internals, and repo-local support code
- `crates/*/tests/**`, `tests/support/**`, runtime fixtures, CI orchestration, local scripts, and benchmark harnesses
- local development, planning, review, and continuity materials
- A2 and any wider handshake/session/capability semantics
- client SDK, provider hosting, gateway runtime, discovery, routing, retry, reconnect, or runtime-policy convenience layers

If something is merely visible because of current repository layout but is not listed in the stable section above, treat it as internal or experimental.

## Legacy Guard: What We Explicitly Do Not Introduce

Current MVP explicitly does not introduce:

- capability negotiation
- preferred-server or redirect semantics
- heartbeat interval or session-timeout negotiation
- reconnect / retry / backoff policy
- runtime discovery / routing / provider policy
- gateway-hosted runtime convenience layers
- client SDK or provider-hosting product surfaces
- token/session semantics that reinterpret handshake freshness markers as authentication secrets
- stable rejection semantics for early malformed `Hello` before new authority ratifies them

## Release Smoke Gate

The current MVP release-operations gate includes a reproducible release smoke path:

- `powershell -ExecutionPolicy Bypass -File .\scripts\release-smoke.ps1`
- `bash ./scripts/release-smoke.sh`

This gate proves only that:

- `laic` and `laicc` can be packaged as official artifacts
- the `laicc` CLI entry point can be invoked
- the minimal Rust / Python / TypeScript generation path succeeds against `crates/laicc/tests/fixtures/echo.laic`

This gate does not widen the stable surface into runtime, discovery, routing, provider hosting, or client SDK capability.

For the current MVP line, the following smoke failures are release-blocking:

- `cargo package -p laic --allow-dirty`
- `cargo package -p laicc --allow-dirty`
- `cargo run -p laicc -- --help`
- any missing Rust / Python / TypeScript output expected by `scripts/release-smoke.*`

## Semver And Breaking-Change Policy

Only the stable surface listed in this file is compatibility-protected.

The following count as breaking changes:

- removing, renaming, retyping, or materially changing the meaning of listed stable `laic` crate-root APIs
- removing, renaming, retyping, or materially changing the meaning of listed stable `laicc` library APIs
- changing `laicc` CLI argument meaning or generated output naming semantics
- renumbering published `ErrorCode` values or reclassifying their stable protocol meaning
- changing the ratified minimal handshake shape split or `client_nonce` echo rule
- changing `.laic` contract semantics in a way that breaks already-ratified cross-language compatibility expectations

The following do not count as stable-surface promises by themselves:

- internal refactors
- module moves beneath stable crate-root entry points
- test harness, CI, or fixture layout changes
- experimental/internal surfaces listed above

Compatibility policy for the current MVP line:

- patch releases must not contain silent breaking changes to the stable surface
- any intentional stable-surface break must be explicitly called out in `CHANGELOG.md` and release notes
- any material expansion or contraction of the stable surface must also be recorded in `CHANGELOG.md`
- before `1.0`, the team still treats the stable surface listed here as protected; breaking it requires explicit re-ratification, not an accidental drift under routine maintenance
- after `1.0`, stable-surface breaks require a major-version boundary
