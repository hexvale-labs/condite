#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build --locked -p condite-ctl
# Cargo resolves config files and CARGO_TARGET_DIR, including paths with spaces.
target_dir=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
export PATH="$target_dir/debug:$PATH"
printf 'Temporary Condite shell: condite-ctl is on PATH. Type exit to leave.\n' >&2
exec "${SHELL:-/bin/bash}" -i
