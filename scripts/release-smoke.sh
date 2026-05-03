#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
smoke_root="$repo_root/.tmp/release-smoke"

cd "$repo_root"
rm -rf "$smoke_root"

echo "Release smoke: package official artifacts, verify CLI entry point, and generate minimal bindings."
echo "This smoke does not prove runtime, discovery, routing, provider hosting, or client SDK behavior."

cargo package -p latrix-laic --allow-dirty
cargo package -p laicc --allow-dirty
cargo run -p laicc -- --help
cargo run -p laicc -- crates/laicc/tests/fixtures/echo.laic --lang rust -o "$smoke_root/rust"
cargo run -p laicc -- crates/laicc/tests/fixtures/echo.laic --lang python -o "$smoke_root/python"
cargo run -p laicc -- crates/laicc/tests/fixtures/echo.laic --lang typescript -o "$smoke_root/typescript"

test -f "$smoke_root/rust/echo_laic.rs"
test -f "$smoke_root/python/echo_laic.py"
test -f "$smoke_root/typescript/echo_laic.ts"

echo "Release smoke passed."
