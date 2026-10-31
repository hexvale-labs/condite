[working-directory: "crates/condite-core"]
migrate name:
    cargo run --bin migrate -- migration generate --name "{{name}}"

[working-directory: "crates/condite-core"]
run:
    cargo run

# Run formatting, clippy, and the Rust test suite.
check-rust:
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

# Lint browser-only Rust code, which native workspace Clippy cannot compile.
check-sdk-wasm:
    cargo clippy --locked -p condite-sdk-wasm --target wasm32-unknown-unknown --all-features -- -D warnings

# Build generated bindings before checking their TypeScript declarations.
check-sdk:
    bun run build:sdk
    bun run check:consumer
    bun run lint:sdk
    bun run test:sdk
    just check-sdk-wasm
