#!/usr/bin/env bash
set -euo pipefail
# Audit enabled dependencies, not merely packages left unused in Cargo.lock.
tree=$(cargo tree --locked --workspace --all-features --prefix none --format '{p}')
if printf '%s\n' "$tree" | rg '^(native-tls|openssl|openssl-sys) v'; then
    echo 'Native TLS/OpenSSL found; use Rustls instead.' >&2
    exit 1
fi
