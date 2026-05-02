# laicc

`laicc` is the LAIC IDL compiler package for Rust, Python, and TypeScript contract bindings.
The package provides both the `laicc` library workflow and the `laicc` CLI.
It does not provide runtime scaffolding, provider hosting, or project-generation workflows.

## Install

Use the library from Rust:

```powershell
cargo add laicc
```

Install the CLI:

```powershell
cargo install laicc
```

## Stable Surface Summary

The current MVP stable library workflow is:

- `compile()`
- `generate_rust()`
- `generate_python()`
- `generate_typescript()`

The current MVP stable CLI contract is:

```text
laicc [--lang rust|python|typescript] [-o <output-dir>] <input>
```

This includes:

- one required input `.laic` file
- optional `--lang` target selection for `rust`, `python`, and `typescript`, with current default `rust`
- optional `-o` / `--output` directory selection, with current default `.`
- generated file naming in the form `<stem>_laic.rs|py|ts`

## Minimal CLI Example

Given a local contract file such as `./echo.laic`:

```powershell
laicc ./echo.laic --lang rust -o ./generated
```

Expected output:

- generated file: `./generated/echo_laic.rs`

## What This Package Does Not Promise

This package does not currently promise:

- config-file loading
- preset or profile systems
- runtime scaffolding
- package publishing helpers
- project-generation workflows
- client SDK or provider-hosting product layers

## Release-Facing References

- Repository: <https://github.com/latticematrix/laic>
- Boundary: <https://github.com/latticematrix/laic/blob/main/docs/BOUNDARY.md>
- Stability contract: <https://github.com/latticematrix/laic/blob/main/docs/STABILITY.md>
- Changelog: <https://github.com/latticematrix/laic/blob/main/CHANGELOG.md>

## License

Licensed under the Apache License, Version 2.0.
