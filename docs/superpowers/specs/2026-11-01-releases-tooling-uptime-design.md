# Releases, CLI tooling and live uptime

## Intent

Make local administration available alongside development services, publish small
production images on version tags, version components independently, and replace
raw ISO durations with a playful, readable live uptime display across clients.

## Approaches

Selected by the owner: component tags plus umbrella releases. Alternatives were
umbrella tags only (simpler, but no independent release trigger) and component
tags only (independent releases, but no single full-stack release).

Implementation choices below are recommendations for approval, not existing behavior.

## Versions and triggers

- One root version manifest records umbrella, core, SDK, dashboard and Vencord
  versions. CLI follows core. All SDK crates and generated JS packages share the
  SDK version; preserve existing crate licenses and dependency boundaries.
- A version tool updates manifests, local dependency version requirements and
  lockfiles together; its check mode rejects drift in CI. Extend existing just
  verbs for version operations without duplicate command aliases.
- Strict SemVer validation follows broad GitHub tag filters. Reject malformed
  tags and mismatched manifest versions before building or publishing.
- Push `core/vX.Y.Z` builds core; `dashboard/vX.Y.Z` builds dashboard;
  `sdk/vX.Y.Z` validates and packages SDKs without introducing registry publishing.
  Push `vX.Y.Z` builds both images using component versions from that commit.
- Images: `ghcr.io/hexvale-labs/condite/core` and
  `ghcr.io/hexvale-labs/condite/dashboard`. Component versions are image version
  tags; umbrella builds additionally get an unambiguous `release-X.Y.Z` tag.
  Include source revision and component version OCI labels. Prereleases do not
  advance stable aliases; refuse overwriting existing version tags with a
  different source revision. Umbrella releases reuse an existing component-version
  image digest and add the umbrella alias instead of rebuilding that published
  component. If a component changed, its version must be bumped first; CI checks
  component inputs (including shared code, locks and build configuration) against
  the recorded source revision. A missing component image is built from the
  umbrella commit. This permits releasing unchanged components without version churn.
- Only a published GitHub release with a valid matching `vX.Y.Z` or
  `vencord/vX.Y.Z` tag builds and attaches Vencord ZIPs. Branch pushes, bare
  Vencord tag pushes and unrelated component releases do not attach assets.

## Runtime images and CI

- Multi-stage Debian builds and nonroot distroless runtime images. Select the
  smallest compatible core runtime after inspecting linked libraries; retain
  glibc for the database/native dependencies. Dashboard uses distroless Node 22
  with a matching Debian build environment, production dependencies and generated
  native SDK only. No compilers, development dependencies or package managers in
  runtime images. No speculative Node heap flags or process managers.
- Build Linux amd64 and arm64 on native runners, merge manifests after both pass.
  Build dependencies separately from source where practical. Use scoped BuildKit
  intermediate-layer caches; do not assume cache mounts persist on hosted runners.
- Strip Rust release symbols and use a dedicated optimized container profile;
  measure final image sizes and build time rather than claiming unmeasured gains.
- Audit enabled TLS features across core and SDKs, keeping Rustls and excluding
  native TLS/OpenSSL. Keep inbound TLS termination external unless already supported.
- Core includes condite-ctl plus executable symlinks `ctl`, `control`, `condite`,
  `condite-control` on PATH. Direct compose exec requires no shell. Prepare
  writable database/socket directories owned by the runtime UID and document
  persistent storage, socket access and environment configuration in compose.
- Docker context excludes credentials, local databases, checkouts and artifacts.
  CI uses minimum job permissions, immutable action references where practical,
  locked dependencies and tagged-source checkout. Smoke-test images before push:
  CLI help through every alias, runtime linking, and dashboard startup/health.

## Local commands

- Extend general install/build/run/check commands with `ctl`; check runs existing
  CLI tests plus formatting and Clippy. Include CLI in all-component operations.
- `just run` retains backend/dashboard in mprocs and adds an interactive shell
  process with the locally built condite-ctl on PATH. Build before opening it;
  PATH changes stay in the child shell and end with the session. Provide explicit
  `just run ctl` for the same shell. Keep bare `just` showing beginner help.
- Ensure the shell and backend agree on the development control socket. Update
  help, diagnostics and component documentation together.

## Vencord packages

- Inspect pinned upstream build scripts and match distributable desktop,
  standalone/Vesktop, browser-extension and userscript targets where supported.
  Package native desktop variants on their actual OS/architecture; use native
  Linux x64/arm64, Windows x64/arm64 and macOS x64/arm64 where upstream and the
  addon support them. Verify runner availability before finalizing the matrix.
- Native addon and its MIT license remain beside desktop bundles. Preserve GPL
  notices for the plugin. Use ZIP deflate level 6, platform-specific asset names,
  checksums and a manifest identifying component versions and source revision.
- The current web build only compiles UI: it cannot run the Electron bridge.
  Implement a browser-compatible SDK path where possible; otherwise document the
  exact unsupported capabilities and do not label a nonfunctional build supported.
  Never silently omit a target or ship an incompatible native addon.

## Uptime experience

- Keep wire types unchanged. Derive uptime from the sampled duration with a
  monotonic local elapsed clock, using started_at as a fallback. Avoid server/client
  clock skew, fractional seconds and rendering raw PT strings. Invalid values
  show unknown; unhealthy/unreachable states never imply green availability.
- Shared formatter emits months (documented as 30 days), days, hours, minutes
  and seconds. Each client updates once per second while mounted and stops its
  timers when removed. CLI remains a readable static snapshot.
- Dashboard and both Vencord integrations get tabular digits with a short rolling
  number transition, a softly pulsing green availability dot, and playful wording
  that suggests a doomsday clock counting upward. Reserve space to prevent jumps.
- A clearly labeled mute toggle starts muted on each mount. User activation
  enables a quiet synthesized clock tick; stop audio when muted, hidden,
  disconnected or unmounted. Never autoplay or persist an unmuted preference.
- Respect reduced motion, keyboard access and localized labels. Do not announce
  every second to screen readers. No audio asset download is needed.

## Validation and limits

Run focused CLI tests, version/tag fixtures, shell syntax/command dispatch checks,
workflow validation and container smoke tests where Docker is available. Run
workspace Rust checks for Rust changes; SDK checks for generated packages;
dashboard type/build/unit/browser checks and Vencord fixtures/upstream checks.
Test uptime rollover, invalid input, mounting cleanup, default mute, user activation,
and reduced motion. Record any unavailable platform/browser/hardware validation.

No real releases, tags or package publication are created while implementing.
Sharing/storage contracts and unrelated code stay unchanged.

## Research

- https://docs.docker.com/build/ci/github-actions/cache
- https://docs.docker.com/build/cache/backends/gha/
- https://github.com/GoogleContainerTools/distroless
- https://github.com/nodejs/docker-node
- Pinned Vencord package scripts and release build workflow at revision
  `8d64c8be7e68616f8f116a6fff9480051aa2a0c5`.
