# Releases, tooling and uptime implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver local CLI tooling, independently versioned tagged releases,
small GHCR images, platform-specific Vencord ZIPs and readable live uptime.

**Architecture:** A root version manifest drives package versions and release
validation. Separate release workflows consume validated tag metadata. Shared
client uptime utilities feed framework-specific displays without wire changes.

**Tech Stack:** Rust, Bash/Just/mprocs, Node/Bun, GitHub Actions/BuildKit,
Astro/Solid 2 and Vencord React.

**Spec:** [Approved design](../specs/2026-11-01-releases-tooling-uptime-design.md).

## Global constraints

- Tags use `core/vX.Y.Z`, `sdk/vX.Y.Z`, `dashboard/vX.Y.Z`,
  `vencord/vX.Y.Z` and umbrella `vX.Y.Z`; Git forbids colons in tags.
- Core/CLI, SDKs, dashboard and Vencord version independently.
- Images publish only on valid version tag pushes; Vencord assets attach only
  on published releases for matching umbrella or Vencord tags.
- Preserve crate license boundaries, bigint precision and sharing contracts.
- Default `just` remains help. Extend general verbs with spaced arguments.
- Rustls everywhere; no native TLS/OpenSSL. Native image builds for amd64/arm64.
- Sound starts muted each mount; reduced motion disables decorative motion.
- Do not create actual tags, releases or publish packages during development.

## Review focus

- Existing component versions in later umbrella releases must reuse immutable
  image digests, while changed component inputs require a version bump (Task 2/4).
- Shell must use the backend's control socket and cleanly exit with mprocs (Task 1).
- Dashboard native SDK must resolve inside runtime without workspace symlinks
  escaping the copied filesystem (Task 3).
- Vencord web artifacts must not silently invoke the Electron bridge (Task 5).
- Offline/hidden/unmounted uptime surfaces must stop sound and never show false
  green availability; hydration must not mismatch the SSR snapshot (Task 6).

### Task 1: CLI commands and development shell

**Files:** `justfile`, `scripts/dev/{task.sh,doctor.sh,help.txt}`,
`mprocs.yaml`, new `scripts/dev/ctl-shell.sh`,
`crates/condite-ctl/justfile`, `crates/condite-ctl/README.md`,
new `scripts/dev/task.test.mjs`, `docs/development.md`.

**Interfaces:** `just install|build|run|check ctl`; `just run ctl` opens a
child shell with built CLI on PATH. Existing all-target behavior includes CLI.

- [ ] Add command-dispatch integration fixtures using stub tools and temporary
  PATH; assert build/check/install routes, all-target CLI inclusion, child-only
  PATH, nondefault CARGO_TARGET_DIR, and matching development socket path.
- [ ] Run `node --test scripts/dev/task.test.mjs` and observe missing behavior.
- [ ] Add local CLI justfile, focused fmt/Clippy/tests, read-only diagnostics,
  and shell script using the actual Cargo target directory. Add a CLI shell
  process to mprocs; preserve shell exit status and cleanup temporary resources.
- [ ] Update help and READMEs with exact commands and mprocs shell interaction.
- [ ] Verify dispatch tests, `cargo test --locked -p condite-ctl`, shell syntax,
  and `just --list`; commit CLI tooling as one focused change.

### Task 2: Unified versions and release routing

**Files:** new `versions.json`, `scripts/release/{version.mjs,version.test.mjs}`,
root/crate Cargo manifests, package manifests, generated package templates in
`scripts/sdk/build.mjs`, lockfiles, `justfile`, `scripts/dev/task.sh`,
new `docs/releases.md`.

**Interfaces:** `node scripts/release/version.mjs check`, `set COMPONENT VERSION`,
`tag TAG` emitting JSON `{component,version,core,sdk,dashboard,vencord,umbrella}`;
components are `core|sdk|dashboard|vencord|umbrella`. Just exposes `check versions`
and `version COMPONENT VERSION` as a single general verb with spaced arguments.

- [ ] Add fixtures asserting strict SemVer (including prerelease/build metadata),
  invalid tag rejection, component/umbrella manifest mismatch, all SDK version
  alignment, generated package versions, and idempotent updates.
- [ ] Observe RED via `node --test scripts/release/version.test.mjs`.
- [ ] Implement manifest synchronization without changing licenses. Core version
  owns CLI; SDK version owns SDK/shared/metadata packages and local requirements;
  dashboard and both Vencord integrations receive their independent versions.
  Refresh Cargo/Bun and independent-consumer lockfiles through their package tools.
- [ ] Add reusable release-input inventory: component source plus shared inputs,
  locks and build config. Compare against previously published image provenance
  to reject changed inputs with an unchanged version. Test unrelated-source
  changes, shared-source changes and first-publication paths with local git fixtures.
- [ ] Verify fixtures and `node scripts/release/version.mjs check`; document
  tags and immutable component reuse; commit the versioning tool.

### Task 3: Minimal production containers

**Files:** new `docker/{core.Dockerfile,dashboard.Dockerfile,compose.yaml}`,
`.dockerignore`, `Cargo.toml`, relevant TLS dependency manifests,
new `scripts/release/smoke-images.sh`, `docs/releases.md`.

**Interfaces:** root-context Docker builds; images run nonroot, core exposes
`condite-ctl|ctl|control|condite|condite-control`; dashboard starts Node standalone
on a documented port, using server-side CONDITE_API_URL and built host validation.

- [ ] Inspect current AWS/HTTP/database enabled features with `cargo tree -e
  features`; consult current upstream docs before changing TLS or build APIs.
- [ ] Add smoke script that asserts all aliases execute help, inspects linked
  libraries and UID, and starts dashboard against a local API fixture. Run it
  against absent/unbuilt images to establish the failure path.
- [ ] Build matching Debian builder/runtime stages, optimized stripped Rust
  container profile, correct writable data/socket paths, and symlink aliases.
  Use smallest distroless runtime supported by actual linked libraries.
- [ ] Build dashboard native/WASM SDKs, Astro output and production dependencies;
  materialize workspace packages so runtime resolution stays inside the image.
  Keep native addon server-only and use distroless Node 22. Exclude environment
  files, local databases and generated artifacts from context.
- [ ] Verify TLS feature tree, smoke tests, built host validation and graceful
  termination; record image sizes. Document unavailable Docker tests explicitly.
  Commit containers and compose usage.

### Task 4: Tagged GHCR publishing and caching

**Files:** new `.github/workflows/images.yml`,
`scripts/release/{images.mjs,images.test.mjs}`, `docs/ci.md`, `docs/releases.md`.

**Interfaces:** tag validation from Task 2; Dockerfiles/smoke checks from Task 3.
Publishes `ghcr.io/hexvale-labs/condite/{core,dashboard}` by component version,
plus `release-X.Y.Z` aliases for umbrella releases.

- [ ] Test routing (branch/malformed/sdk/Vencord/component/umbrella tags), immutable
  image reuse, missing images, changed inputs and prerelease alias exclusion.
- [ ] Observe RED with `node --test scripts/release/images.test.mjs`.
- [ ] Implement native amd64/arm64 matrix, per-image/per-architecture BuildKit
  cache scopes with intermediate stages, smoke-before-push, digest artifacts and
  final multiarch manifest publication. Existing version tags reuse verified
  digests; never overwrite a differing component version. Authenticate GHCR with
  minimal job permissions and use validated metadata in shell arguments.
- [ ] Add source/version labels and digest/size summary. Pin actions after checking
  current upstream releases. Run workflow lint and all release fixtures; commit CI.

### Task 5: Vencord release ZIPs

**Files:** `clients/vencord-extension/tooling.mjs`, new
`scripts/release/{vencord.mjs,vencord.test.mjs}`, new
`.github/workflows/vencord-release.yml`, both Vencord READMEs and browser bridge
files as required by pinned upstream target inspection.

**Interfaces:** published-release routing from Task 2; platform-specific ZIPs
with native addon, licenses, versions, checksums and source revision manifest.

- [ ] Inspect pinned upstream desktop/standalone/web/extension/userscript outputs,
  current native SDK targets and GitHub runner availability; define supported matrix.
- [ ] Add release fixture assertions for invalid/unrelated tag exclusion, output
  completeness, per-platform addon identity, license inclusion and ZIP deflate 6.
- [ ] Observe RED; implement isolated output staging per target and release uploads
  only after required matrix jobs succeed. Avoid a native build overwriting another
  variant; keep addon beside desktop bundles and build native on target hosts.
- [ ] Implement browser SDK bridge if feasible within the pinned integration;
  test browser health/upload paths without Electron. If impossible, record precise
  unsupported capabilities and do not publish broken targets as usable packages.
- [ ] Verify fixture suites, Vencord types/lint/builds and available native variants;
  document unavailable live Discord/OS checks. Commit Vencord release automation.

### Task 6: Shared uptime and client displays

**Files:** new `clients/shared/{uptime.ts,uptime.test.ts,tick.ts}`,
`clients/dashboard/src/components/{Service.tsx,Uptime.tsx}`,
`clients/dashboard/src/styles/global.css`, both Vencord managers/styles and new
Uptime React components, shared translations, relevant test/build configurations,
CLI status formatting in `crates/condite-ctl/src/` and its existing tests.

**Interfaces:** `sampleUptime(runtime,startedAt,wallNow,monotonicNow)` creates a
validated snapshot; `elapsedSeconds(snapshot,monotonicNow)` returns nonnegative
whole seconds or null; `splitUptime(seconds)` yields months/days/hours/minutes/
seconds with 30-day months. `createTickPlayer()` exposes user-activated enable,
mute and dispose without creating AudioContext until user interaction.

- [ ] Read Solid 2, animation and Rust skills before their implementation work.
  Add duration/fallback/skew/fraction/invalid/rollover fixtures; run RED with
  `bun test clients/shared/uptime.test.ts`. Add CLI readable snapshot regression.
- [ ] Implement shared time math and localized formatting without wire changes.
  Implement audio cleanup and graceful unsupported-browser behavior.
- [ ] Integrate Solid 2 SSR-safe initial snapshot and React displays with one-second
  timers, short rolling digits, reserved width, green pulse only when healthy,
  and localized mute button initially muted. Use sampled monotonic elapsed time;
  stop/reconcile on disconnect, visibility changes and unmount.
- [ ] Add browser assertions for default mute, explicit activation, mute/hidden/
  unmount cleanup, reduced motion, offline state, stable SSR/hydration and keyboard
  access. Avoid live announcements on every tick. Sync shared files via existing
  Vencord build tooling; do not manually copy generated wire types.
- [ ] Run dashboard check/unit/build/browser checks, Vencord suites and CLI tests;
  commit uptime behavior and document real-device/audio validation limits.

### Task 7: Whole-change verification and review

**Files:** component documentation, `docs/{development.md,ci.md,releases.md}`.

**Interfaces:** completed Tasks 1–6.

- [ ] Run `just check rust`, SDK validation, dashboard checks and Vencord checks
  appropriate to changed files; run all release/tooling fixtures and workflow lint.
- [ ] Check relative documentation links, `git diff --check`, artifact/secret
  exclusions and actual supported platform claims against available evidence.
- [ ] Obtain one fresh whole-change code review, fix important findings with focused
  regressions, rerun affected checks, and report limitations without publishing.
