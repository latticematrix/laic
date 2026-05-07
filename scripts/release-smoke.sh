#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
smoke_root=""

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

while [[ $# -gt 0 ]]; do
  case "$1" in
    --smoke-root)
      if [[ $# -lt 2 ]]; then
        echo "missing value for --smoke-root" >&2
        exit 2
      fi
      smoke_root="$2"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$smoke_root" ]]; then
  smoke_root="$repo_root/.tmp/release-smoke/$(date +%Y-%m-%d-%H%M%S)-sh-$$"
elif [[ "$smoke_root" != /* && ! "$smoke_root" =~ ^[A-Za-z]:[\\/].* ]]; then
  smoke_root="$repo_root/$smoke_root"
fi

# Resolve lexically without requiring the smoke directory to exist. The guard is
# intentionally before rm -rf so a typo cannot delete outside the repo-local roots.
# Git Bash needs cygpath for Windows drive paths; Linux CI falls back to realpath.
smoke_root="$(normalize_path "$smoke_root")"
release_smoke_prefix="$(normalize_path "$repo_root/.tmp/release-smoke")/"
usmoke_prefix="$(normalize_path "$repo_root/.tmp/usmoke")/"
case "$smoke_root" in
  "$release_smoke_prefix"*|"$usmoke_prefix"*) ;;
  *)
    echo "SmokeRoot must resolve under repo-local .tmp/release-smoke/ or .tmp/usmoke/: $smoke_root" >&2
    exit 2
    ;;
esac

# Keep cargo package verification out of the workspace target dir; Windows can keep
# target/package files locked between package runs. The per-run root keeps parallel
# reviewer evidence from deleting another smoke process's artifacts.
smoke_target_root="$smoke_root/target"

cd "$repo_root"
rm -rf "$smoke_root"

echo "Release smoke: package official artifacts, verify CLI entry point, and generate minimal bindings."
echo "This smoke does not prove runtime, discovery, routing, provider hosting, or client SDK behavior."
echo "Release smoke artifact root: $smoke_root"

cargo package -p latrix-laic --allow-dirty --target-dir "$smoke_target_root"
cargo package -p laicc --allow-dirty --target-dir "$smoke_target_root"
cargo run --target-dir "$smoke_target_root" -p laicc -- --help
cargo run --target-dir "$smoke_target_root" -p laicc -- crates/laicc/tests/fixtures/echo.laic --lang rust -o "$smoke_root/rust"
cargo run --target-dir "$smoke_target_root" -p laicc -- crates/laicc/tests/fixtures/echo.laic --lang python -o "$smoke_root/python"
cargo run --target-dir "$smoke_target_root" -p laicc -- crates/laicc/tests/fixtures/echo.laic --lang typescript -o "$smoke_root/typescript"

test -f "$smoke_root/rust/echo_laic.rs"
test -f "$smoke_root/python/echo_laic.py"
test -f "$smoke_root/typescript/echo_laic.ts"

echo "Release smoke passed."
