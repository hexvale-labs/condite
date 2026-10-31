# Show available development commands.
default:
    @just --list

# Install backend dependencies and create its local .env if absent.
install-backend:
    just --justfile crates/condite-core/justfile install

# Install SDK tooling, generate packages, and install dashboard dependencies.
install-dashboard:
    just --justfile clients/dashboard/justfile install

# Install dependencies for the shared, Rust, Node, WASM, and consumer SDK crates.
install-sdk:
    just --justfile scripts/sdk/justfile install

# Prepare both applications and generated SDKs for local development.
setup:
    just install all

# Install dependencies for sdk, dashboard, backend, or all packages.
install package:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "==> Install: {{package}}"
    case "{{package}}" in
        sdk) just --justfile scripts/sdk/justfile install ;;
        dashboard) just --justfile clients/dashboard/justfile install ;;
        backend) just --justfile crates/condite-core/justfile install ;;
        all)
            just --justfile crates/condite-core/justfile install
            just --justfile clients/dashboard/justfile install
            ;;
        *) echo 'Usage: just install <sdk|dashboard|backend|all>' >&2; exit 2 ;;
    esac
    echo "==> Install complete: {{package}}"

# Build sdk (including generated files), dashboard, backend, or all packages.
build package:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "==> Build: {{package}}"
    case "{{package}}" in
        sdk) just --justfile scripts/sdk/justfile build ;;
        dashboard) just --justfile clients/dashboard/justfile build ;;
        backend) just --justfile crates/condite-core/justfile build ;;
        all)
            just --justfile scripts/sdk/justfile build
            just --justfile crates/condite-core/justfile build
            echo '==> Refreshing dashboard SDK dependencies'
            bun --cwd clients/dashboard install --frozen-lockfile
            just --justfile clients/dashboard/justfile build
            ;;
        *) echo 'Usage: just build <sdk|dashboard|backend|all>' >&2; exit 2 ;;
    esac
    echo "==> Build complete: {{package}}"

# Start the backend on port 8080.
backend:
    just --justfile crates/condite-core/justfile run

# Start the dashboard on port 4321; run the backend in another terminal.
dashboard:
    just --justfile clients/dashboard/justfile run

# Generate shared types and build all SDK artifacts.
build-sdk:
    just --justfile scripts/sdk/justfile build

# Check all packages, or choose backend, dashboard, or sdk.
doctor package="all":
    bash scripts/dev/doctor.sh "{{package}}"

# Backward-compatible backend command.
run: backend

migrate name:
    just --justfile crates/condite-core/justfile migrate "{{name}}"

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
    just build-sdk
    npm run check:consumer
    npm run lint:sdk
    npm run test:sdk
    just check-sdk-wasm
