#!/usr/bin/env bash
set -euo pipefail
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

run_check "Rust formatting" cargo fmt --all -- --check
run_check "Clippy (all targets and features; warnings are errors)" \
    cargo clippy --workspace --locked --all-targets --all-features -- -D warnings

run_check "Rust tests" cargo test --workspace --locked --all-targets --all-features

exit "$status"
