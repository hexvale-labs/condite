#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest_path="$repo_root/core/Cargo.toml"
status=0

run_check() {
    local description="$1"
    shift

    printf '\n==> %s\n' "$description"
    if "$@"; then
        return 0
    fi

    status=1
}

run_check "Rust formatting" cargo fmt --manifest-path "$manifest_path" -- --check
run_check "Clippy (all targets and features; warnings are errors)" \
    cargo clippy --manifest-path "$manifest_path" --all-targets --all-features -- -D warnings

exit "$status"
