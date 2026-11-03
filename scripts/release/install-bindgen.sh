#!/usr/bin/env bash
set -euo pipefail
version=$1
arch=$2
destination=$3
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Invalid wasm-bindgen version' >&2; exit 2; }
case "$arch" in
    amd64) machine=x86_64 ;;
    arm64) machine=aarch64 ;;
    *) echo 'Unsupported container architecture' >&2; exit 2 ;;
esac
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
cd "$scratch"
name="wasm-bindgen-$version-$machine-unknown-linux-musl"
base="https://github.com/wasm-bindgen/wasm-bindgen/releases/download/$version"
curl --fail --silent --show-error --location --connect-timeout 15 --max-time 120 --retry 2 -O "$base/$name.tar.gz"
curl --fail --silent --show-error --location --connect-timeout 15 --max-time 120 --retry 2 -O "$base/$name.tar.gz.sha256sum"
# Upstream checksum files may include a release-staging directory prefix.
read -r expected checksum_path < "$name.tar.gz.sha256sum"
[[ "$expected" =~ ^[0-9a-fA-F]{64}$ ]] || { echo 'Invalid checksum' >&2; exit 1; }
printf '%s  %s\n' "$expected" "$name.tar.gz" | sha256sum --check
tar -xzf "$name.tar.gz" "$name/wasm-bindgen"
install -m 755 "$name/wasm-bindgen" "$destination/wasm-bindgen"
