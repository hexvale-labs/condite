#!/usr/bin/env bash
set -euo pipefail
component=$1
image=$2
if [[ "$component" == core ]]; then
    for alias in condite-ctl ctl control condite condite-control; do
        docker run --rm --entrypoint "$alias" "$image" --help >/dev/null
    done
    test "$(docker image inspect --format '{{.Config.User}}' "$image")" != root
else
    id=$(docker run -d -p 127.0.0.1::4321 "$image")
    trap 'docker logs "$id"; docker rm -f "$id" >/dev/null' EXIT
    port=$(docker port "$id" 4321/tcp | head -1)
    for attempt in {1..30}; do
        if curl --fail --silent "http://$port/en/" >/dev/null; then exit 0; fi
        sleep 1
    done
    exit 1
fi
