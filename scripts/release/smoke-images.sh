#!/usr/bin/env bash
set -euo pipefail
component=$1
image=$2
runtime_user=$(docker image inspect --format '{{.Config.User}}' "$image")
case "$runtime_user" in ''|root|0|0:*) echo 'Runtime image must run nonroot' >&2; exit 1 ;; esac
if [[ "$component" == core ]]; then
    for alias in condite-ctl ctl control condite condite-control; do
        docker run --rm --network none --entrypoint "$alias" "$image" --help >/dev/null
    done
    # No external storage is contacted: validate startup, migrations and the real
    # private control socket inside an isolated network with fixture credentials.
    id=$(docker run -d --network none \
        -e S3_ENDPOINT=https://storage.example -e S3_REGION=us-east-1 \
        -e S3_BUCKET=fixture-private -e S3_ACCESS_KEY=example -e S3_SECRET_KEY=example "$image")
    trap 'docker logs "$id"; docker rm -f "$id" >/dev/null' EXIT
    for attempt in {1..30}; do
        if docker exec "$id" ctl --json status > /dev/null 2>&1; then exit 0; fi
        sleep 1
    done
    exit 1
elif [[ "$component" == dashboard ]]; then
    if [[ "${SMOKE_HOST_NETWORK:-0}" == 1 ]]; then
        port=${SMOKE_PORT:-14321}
        id=$(docker run -d --network host -e PORT="$port" "$image")
        address="127.0.0.1:$port"
    else
        id=$(docker run -d -p 127.0.0.1::4321 "$image")
        address=$(docker port "$id" 4321/tcp | head -1)
    fi
    trap 'docker logs "$id"; docker rm -f "$id" >/dev/null' EXIT
    for attempt in {1..30}; do
        if curl --max-time 8 --fail --silent "http://$address/en/" > /dev/null; then exit 0; fi
        sleep 1
    done
    exit 1
else
    echo 'Use core or dashboard' >&2
    exit 2
fi
