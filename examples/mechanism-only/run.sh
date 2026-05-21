#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
demo_root="${1:-$repo_root/.tmp/mechanism-only-demo}"

normalize_path() {
  local path="$1"

  if command -v cygpath >/dev/null 2>&1; then
    cygpath -am "$path"
    return
  fi

  if command -v realpath >/dev/null 2>&1; then
    realpath -m "$path"
    return
  fi

  python3 -c 'import os, sys; print(os.path.abspath(sys.argv[1]))' "$path"
}

case "$demo_root" in
  /*|[A-Za-z]:[\\/]*) ;;
  *) demo_root="$repo_root/$demo_root" ;;
esac

demo_root="$(normalize_path "$demo_root")"
tmp_prefix="$(normalize_path "$repo_root/.tmp")/"
case "$demo_root" in
  "$tmp_prefix"*) ;;
  *)
    echo "DemoRoot must resolve under repo-local .tmp/: $demo_root" >&2
    exit 2
    ;;
esac

cd "$repo_root"
rm -rf "$demo_root"

contract="$repo_root/examples/mechanism-only/echo_contract.laic"
target_root="$demo_root/target"

echo "LAIC mechanism-only demo: compile one .laic contract into Rust, Python, and TypeScript bindings."
echo "This demo does not prove runtime, routing, provider hosting, workflow, marketplace, or multi-agent behavior."
echo "Demo output root: $demo_root"

cargo run --target-dir "$target_root" -p laicc -- "$contract" --lang rust -o "$demo_root/rust"
cargo run --target-dir "$target_root" -p laicc -- "$contract" --lang python -o "$demo_root/python"
cargo run --target-dir "$target_root" -p laicc -- "$contract" --lang typescript -o "$demo_root/typescript"

test -f "$demo_root/rust/echo_contract_laic.rs"
test -f "$demo_root/python/echo_contract_laic.py"
test -f "$demo_root/typescript/echo_contract_laic.ts"

echo "LAIC mechanism-only demo passed."
