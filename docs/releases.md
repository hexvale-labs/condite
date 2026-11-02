# Versions and releases

`versions.json` records independent core/CLI, SDK, dashboard and Vencord versions,
plus the umbrella release version. Run `just version COMPONENT X.Y.Z` to update
manifests and lockfiles together; `just check versions` detects drift. Components
are `core`, `sdk`, `dashboard`, `vencord`, `umbrella`. All SDK crates (including
shared wire types and metadata processors) and generated JS packages share `sdk`.
Review and commit the resulting changes before tagging.

| Git tag | Push behavior | Published GitHub release behavior |
| --- | --- | --- |
| `core/v1.2.3` | Build core/CLI image | No Vencord assets |
| `dashboard/v1.2.3` | Build dashboard image | No Vencord assets |
| `sdk/v1.2.3` | Validate/build SDKs; retain CI packages | No registry publishing |
| `vencord/v1.2.3` | No image builds | Attach native Vencord ZIPs |
| `v1.2.3` | Build both images | Attach native Vencord ZIPs |

Tag versions must match their entries in the manifest. Invalid SemVer tags fail
validation before publishing. Colons cannot appear in Git tags; a release title
may use `core:v1.2.3`. Prerelease SemVer is supported. Docker tags encode SemVer
build metadata's `+` as `_` because Docker forbids `+`.

## Images

- `ghcr.io/hexvale-labs/condite/core:CORE_VERSION`
- `ghcr.io/hexvale-labs/condite/dashboard:DASHBOARD_VERSION`

Umbrella releases add `:release-UMBRELLA_VERSION` to each image. Component version
images are immutable: unchanged build inputs reuse their existing digest; changed
inputs require a version bump. Shared sources, dependency locks and build config
count as inputs; the effective dashboard origin is also fingerprinted. No floating `latest` alias is published. Linux amd64 and arm64
build natively, smoke-test before push, then merge into multiarchitecture indexes.
Temporary `build-RUN-ATTEMPT-ARCH` tags hold tested images while the matrix finishes;
GHCR retention should remove old temporary tags after successful publication.

Runtime images are nonroot distroless Debian 13: core uses the C runtime
without OpenSSL plus its required libgcc, and dashboard uses Node 22. Matching Debian builders
avoid Alpine/musl addon compatibility changes. Custom Vencord builds disable the upstream updater so official updates cannot
replace Condite bundles. Upgrade them by installing a newer Condite ZIP.

Rust container builds use thin LTO,
one codegen unit and stripped symbols. Production Node dependencies are installed
separately; no compiler or package manager is shipped. Heap flags and extra process
managers are deliberately avoided without workload measurements. Run one Node
process per container and scale replicas externally. A reverse proxy can handle
TLS/compression and cache fingerprinted static assets; keep authenticated API
responses and SSR pages out of shared caches.

Set repository variable `DASHBOARD_ORIGIN` to the public dashboard origin before
building releases (defaults to `http://localhost:4321`). Astro's allowed host is
compiled into the build; changing only the runtime environment cannot change it.
The API URL and optional API key stay server-side runtime variables.

```sh
# Configure crates/condite-core/.env from its example first.
docker compose -f docker/compose.yaml up -d
docker compose -f docker/compose.yaml exec core condite-ctl status
docker compose -f docker/compose.yaml exec core ctl apikey generate
```

`control`, `condite`, and `condite-control` also execute the same CLI. They are
filesystem symlinks on PATH, so direct compose exec works without a shell. The
core data volume and private runtime socket are owned by UID 65532. Back up the
volume before upgrading; startup applies migrations. Supply S3/database secrets
through runtime environment files or orchestration secrets, never image builds.

Check available CPU, RAM and disk first; use `--build-arg CARGO_BUILD_JOBS=1`
on constrained machines and run builds sequentially.

Local builds: `docker build -f docker/core.Dockerfile -t condite-core .` and
`docker build -f docker/dashboard.Dockerfile --build-arg DASHBOARD_ORIGIN=https://dashboard.example -t condite-dashboard .`.
Run `bash scripts/release/smoke-images.sh COMPONENT IMAGE` before using them.

## Vencord assets

Publishing a GitHub release for a matching umbrella or Vencord SemVer tag builds
desktop and standalone/Vesktop variants on Linux, Windows and macOS, each x64 and
arm64. Each ZIP contains the matching native addon beside the bundles, GPL/MIT
notices, component/source metadata and installation instructions. Keep the whole
extracted directory together. Deflate level 6 balances size and compression time;
SHA-256 sidecars verify downloads. The final upload waits for the entire matrix.

Browser, Chromium/Firefox extension and userscript targets are **unsupported by
the current Condite integration**, even though upstream Vencord builds them.
Condite uses Electron IPC and an OS-specific N-API SDK; its browser SDK requires
separate WASM/assets, worker hosting and Discord CSP qualification. `build:web`
checks UI compilation only and is not published as a working integration. Desktop
packages retain the full existing API surface; no browser package silently falls
back to incomplete HTTP/Number conversions. See the [userplugin guide](../clients/vencord-extension/README.md).

## Build research

[Distroless](https://github.com/GoogleContainerTools/distroless) omits shells and
package managers. [Node's image guidance](https://github.com/nodejs/docker-node)
describes Alpine's musl difference. [Cargo-chef](https://github.com/LukeMathWalker/cargo-chef)
separates native/WASM Rust dependency compilation from source layers. Dashboard
builds download the lockfile-matched wasm-bindgen CLI with checksum verification
instead of compiling it from source. [Docker cache guidance](https://docs.docker.com/build/ci/github-actions/cache)
explains intermediate-layer export: workflows use separate `gha` scopes per image
and architecture with `mode=max`. Cache mounts alone are not exported. Enabled
core dependencies use Rustls; [AWS documents Rustls as its default HTTPS client](https://docs.aws.amazon.com/sdk-for-rust/latest/dg/http.html).
