#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
action=$1
target=$2
component=${3:-all}
local_task() { just --justfile "$1/justfile" "$action"; }
if [[ "$action" == check && "$target" == tools ]]; then
    bash scripts/dev/doctor.sh "$component"
    exit
fi
if [[ "$component" != all ]]; then
    echo 'Only check tools accepts a component argument. See just help.' >&2
    exit 2
fi
case "$action:$target" in
    install:all)
        bash "$0" install backend
        bash "$0" install dashboard
        bash "$0" install vencord
        bash "$0" install runner ;;
    install:runner)
        command -v mprocs >/dev/null || cargo install mprocs --locked ;;
    build:all)
        bash "$0" build sdk
        bash "$0" build backend
        bash "$0" build dashboard
        bash "$0" build vencord ;;
    run:all)
        if ! command -v mprocs >/dev/null; then
            echo 'Install the local process runner with: just install runner' >&2
            exit 1
        fi
        exec mprocs --config mprocs.yaml ;;
    run:sdk|run:vencord)
        echo 'SDK and Vencord have no standalone server. Use just check sdk or just build vencord.'
        echo 'For live Vencord testing, inject the build and restart Discord; see clients/vencord-extension/README.md.' >&2
        exit 2 ;;
    check:all)
        bash "$0" check rust
        bash "$0" check sdk
        bash "$0" check dashboard
        bash "$0" check vencord ;;
    check:rust|check:backend) bash scripts/dev/check-rust.sh ;;
    check:wasm)
        cargo clippy --locked -p condite-sdk-wasm --target wasm32-unknown-unknown --all-features -- -D warnings ;;
    install:backend|build:backend|run:backend) local_task crates/condite-core ;;
    install:dashboard|build:dashboard|run:dashboard|check:dashboard) local_task clients/dashboard ;;
    install:sdk|build:sdk|check:sdk) local_task scripts/sdk ;;
    install:vencord|build:vencord|check:vencord) local_task clients/vencord-extension ;;
    *) echo "Unknown task: $action $target. See just help." >&2; exit 2 ;;
esac
