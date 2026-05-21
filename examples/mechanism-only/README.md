# LAIC Mechanism-Only Demo

This demo shows the smallest LAIC-owned path that is useful from a clean
checkout: a local `.laic` contract compiled by `laicc` into Rust, Python, and
TypeScript bindings.

It is intentionally not a runtime, routing, provider-hosting, workflow,
marketplace, or multi-agent demo. It does not prove transport performance,
application policy, service discovery, inference routing, or production SLA
behavior.

## What It Proves

- the repository can invoke `laicc`;
- a self-contained `.laic` contract parses and validates;
- Rust, Python, and TypeScript generated binding files are produced;
- generated output paths and file names match the current `laicc` CLI contract.

## Run It

From the repository root, run one of these commands.

PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -File .\examples\mechanism-only\run.ps1
```

Git Bash or Linux shell:

```bash
bash ./examples/mechanism-only/run.sh
```

Expected generated files:

- `.tmp/mechanism-only-demo/rust/echo_contract_laic.rs`
- `.tmp/mechanism-only-demo/python/echo_contract_laic.py`
- `.tmp/mechanism-only-demo/typescript/echo_contract_laic.ts`

The `.tmp/mechanism-only-demo/` directory is generated output and should not be
committed.

## Boundary

This demo is only a public-adoption smoke path for LAIC's contract and code
generation mechanism. It does not introduce a new stable surface or release
artifact by itself, and it does not trigger a new version.
