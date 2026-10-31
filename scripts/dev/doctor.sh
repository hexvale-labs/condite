#!/usr/bin/env bash
# Read-only prerequisite checks; never print configuration values.
set -uo pipefail
cd "$(dirname "$0")/../.."
status=0

ok() { printf 'OK    %s\n' "$1"; }
missing() { printf 'ERROR %s\n' "$1"; status=1; }
tool() {
    if command -v "$1" >/dev/null 2>&1; then ok "$1";
    else missing "$1 is missing. $2"; fi
}
file() {
    if test -e "$1"; then ok "$1";
    else missing "$1 is missing. $2"; fi
}
version() {
    local actual
    if ! command -v "$1" >/dev/null 2>&1; then missing "$1 is missing. Install $1 >= $2."; return; fi
    actual=$("$1" --version)
    actual=${actual#v}
    if test "$(printf '%s\n%s\n' "$2" "$actual" | sort -V | head -n 1)" = "$2"; then
        ok "$1 $actual"
    else missing "$1 >= $2 is required (found $actual)."; fi
}
backend() {
    printf '\nBackend\n'
    tool cargo 'Install Rust using rustup.'
    tool rustc 'Install Rust using rustup.'
    local key
    for key in SQL_CONN_URL S3_ENDPOINT S3_REGION S3_BUCKET S3_ACCESS_KEY S3_SECRET_KEY; do
        if test -n "${!key:-}" || { test -f crates/condite-core/.env && grep -Eq "^[[:space:]]*(export[[:space:]]+)?${key}=[[:space:]]*[^[:space:]#]" crates/condite-core/.env; }; then
            ok "$key is configured (value not validated)"
        else missing "Set $key in crates/condite-core/.env or your environment. See .env.example."; fi
    done
    printf 'Database connectivity and S3 credentials are not probed.\n'
}
sdk() {
    printf '\nSDK\n'
    tool cargo 'Install Rust using rustup.'
    tool rustc 'Install Rust using rustup.'
    tool rustup 'Install Rust using rustup.'
    version node 22.0.0
    version bun 1.4.4
    tool cc 'Install a C compiler/toolchain.'
    file node_modules/.bin/napi 'Run just install-sdk.'
    if command -v rustup >/dev/null 2>&1 && rustup target list --installed | grep -Fxq wasm32-unknown-unknown; then
        ok wasm32-unknown-unknown
    else missing 'WASM target missing. Run just install-sdk.'; fi
    local expected actual
    expected=$(awk '/^name = "wasm-bindgen"$/ {found=1; next} found && /^version =/ {gsub(/"/, "", $3); print $3; exit}' Cargo.lock)
    actual=$("${WASM_BINDGEN:-wasm-bindgen}" --version 2>/dev/null || true)
    if test "$actual" = "wasm-bindgen $expected"; then ok "$actual";
    else missing "wasm-bindgen CLI must match Cargo.lock ($expected). Run just install-sdk; check WASM_BINDGEN if set."; fi
}
dashboard() {
    printf '\nDashboard\n'
    version node 22.23.4
    version bun 1.4.4
    file frontends/dashboard/node_modules/.bin/astro 'Run just install-dashboard.'
    file crates/condite-sdk-node/dist/condite-sdk.node 'Run just build-sdk (or just install-dashboard).'
    file crates/condite-sdk-node/dist/package.json 'Run just build-sdk.'
    file crates/condite-sdk-wasm/dist/condite_sdk_bg.wasm 'Run just build-sdk.'
    file crates/condite-sdk-wasm/dist/package.json 'Run just build-sdk.'
    printf 'Backend defaults to http://127.0.0.1:8080; override CONDITE_API_URL in frontends/dashboard/.env.\n'
}
case "${1:-all}" in
    backend) backend ;;
    sdk) sdk ;;
    dashboard) dashboard ;;
    all) backend; sdk; dashboard ;;
    *) printf 'Usage: just doctor [all|backend|dashboard|sdk]\n' >&2; exit 2 ;;
esac
exit "$status"
