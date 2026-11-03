# API delivery, administration and TOML implementation plan

> **For agentic workers:** Use superpowers:executing-plans to implement approved tasks sequentially. Approved for implementation by the user on 2026-11-03; execution decisions are recorded in the design and progress ledger.

**Goal:** Deliver validated API downloads, key-based dashboard accounts, remote administration, universal TOML configuration and visible client parity.

**Architecture:** Root-only HTTP administration and Unix control share core's command bus. Core authenticates ordinary/root keys and persistent opaque sessions. Clients use shared wire contracts and preserve ownership and bigint precision.

**Tech stack:** Rust/Axum/Serde/toml, Toasty, S3, existing SDK bindings, Astro/Solid v2, Vencord React integrations.

**Spec:** [Implemented design](../specs/2026-11-03-api-streaming-dashboard-login-design.md).

Status: implemented and locally verified on 2026-11-03. Checkboxes record implementation; provider/live-client checks and validation limits are recorded in the design below.

## Constraints

- Multiple named TOML root credentials; ordinary keys remain isolated.
- TOML throughout; environment values are explicit substitutions only.
- SDK crates stay independent of core/AWS/database dependencies.
- Do not rewrite applied migrations or discard durable object cleanup records.
- Shared types first; regenerate bindings; JS byte quantities remain bigint.
- Read component guidance and check machine capacity before heavy validation.
- Run compiler-heavy checks sequentially; reuse validated artifacts.
- Update docs/client-capabilities.md with each implemented capability change.

## Review focus

- Credential rotation/removal and revoked ordinary keys invalidate existing sessions.
- Environment values containing quotes/newlines never create TOML structure or leak in errors.
- Cross-key IDs, pagination and root filters cannot leak ordinary owners' data.
- Interrupted streams, invalid ranges and upstream failures stay bounded and release resources.
- Legacy direct URLs and late public copies remain tracked until storage cleanup succeeds.

Execute these three tasks in order; each includes its own tests and documentation updates. Task 2 consumes Task 1’s validated configuration and authorization model. Task 3 consumes Task 2’s API and SDK interfaces.

## Task 1: TOML configuration, root authentication and usage

**Lock contracts and configuration semantics**

**Files:** crates/condite-shared/src/{lib.rs,control.rs}; new configuration module/crate after inventory; Cargo.toml; docker/compose.yaml; component READMEs.

- [x] Inventory configuration readers across core, dashboard, ctl, SDK tooling and both Vencord integrations; distinguish deploy settings, client preferences and platform variables.
- [x] Resolve the open questions in the spec and mark design approved before product implementation.
- [x] Define account identity/role, usage and quota DTOs; optional quotas mean unlimited, never a magic maximum integer. Usage exposes confirmed bytes, pending reserved bytes and total charged bytes.
- [x] Define paginated key summaries (ID, optional label, limits, usage, creation/revocation state; no digest/secret), root owner filters, login/session endpoints and administrative errors.
- [x] Settle streamed download compatibility with all SDKs before replacing DownloadResponse. Document whether old API routes are retained or versioned.

**Deliverable:** reviewed contracts/config schema and migration policy; serialization fixtures include values above Number.MAX_SAFE_INTEGER.

**Typed TOML configuration and tooling**

**Files:** crates/condite-core/src/state/{app.rs,config.rs,env.rs}; crates/condite-core/src/logging.rs; crates/condite-ctl/src/main.rs; clients/dashboard/src/lib/server.ts; client native loaders/settings; docker/; scripts/dev/; justfiles.

- [x] Add focused fixtures that fail for missing interpolation variables, duplicate root names/secrets, unknown fields, malformed URLs, invalid sizes and secret-safe diagnostics.
- [x] Implement parse → string substitution → typed validation. Test quotes, newlines, Unicode, literal escaping and nonrecursive substitution.
- [x] Add config path selection, documented defaults and relative-path resolution. Provide committed example TOML and ignored/example .env secret files without reading unrelated environment settings as overrides.
- [x] Implement equivalent dashboard parsing with shared fixtures; keep server configuration out of browser bundles. Implement approved client TOML import/discovery behavior without exposing server secrets.
- [x] Update install/check-tools commands, Docker mounts/environment injection and examples; preserve existing just verb conventions.
- [x] Implement candidate validation and restart-required reporting for reload; test failed reload retaining active settings.

**Deliverable:** all scoped applications boot from TOML; secret interpolation works in Docker and locally. Configuration fixtures pass before auth work begins.

**Root principals and account usage**

**Files:** crates/condite-core/src/auth.rs; crates/condite-core/src/db/sql/{api_key.rs,upload.rs}; new account handler; crates/condite-core/src/lib.rs; generated Toasty migrations; crates/condite-core/tests/uploads.rs.

- [x] Add integration failures for regular-key isolation, multiple roots, root rotation, all-owner pagination and pending/confirmed quota accounting.
- [x] Introduce explicit principal/role authorization; stable root ownership survives rotation. Root bypasses logical quotas while honoring provider and representation constraints.
- [x] Add GET /v1/account with role, account identity, charged/reserved/confirmed usage and limits; root view clearly distinguishes global and own usage.
- [x] Ensure every owner-gated operation uses the new authorization model and never trusts a client role/owner claim. Removing roots must not orphan cleanup.
- [x] Pass auth/usage tests, including concurrent quota reservations and revoked keys.

**Deliverable:** directly authenticated regular/root accounts and accurate usage APIs.

## Task 2: HTTP control, API streaming and SDK contracts

**Remote HTTP control and CLI transport**

**Files:** crates/condite-core/src/control/{processor.rs,bus.rs}; new administrative handlers; crates/condite-shared/src/control.rs; crates/condite-ctl/; crates/condite-core/tests/.

- [x] Test missing/regular/root authorization for every command and malformed/oversized request handling.
- [x] Expose versioned HTTP/JSON administration on core's configured API port, delegating existing operations to CommandBus; retain local Unix socket permissions and framing.
- [x] Add paginated key listing/inspection and usage summaries to shared control operations. Do not serialize generated key secrets into logs.
- [x] Add authenticated HTTP transport to ctl, using TOML credentials and retaining Unix transport. Enforce HTTP timeouts and TLS for remote use; local Docker network HTTP remains documented.
- [x] Verify shutdown returns acknowledgement before server draining; reload and log-level operations report actual results.

**Deliverable:** dashboard-ready remote control and Unix/HTTP CLI parity, with no unauthenticated control port.

**API streaming and public-copy retirement**

**Files:** crates/condite-core/src/v1/uploads/{download.rs,shares.rs}; S3 helpers; SQL cleaner/share records; router; crates/condite-core/tests/uploads.rs; docs/file-visibility-architecture.md.

- [x] Add fake-S3/router tests for public/owner bytes, expiration, revocation, HEAD, valid/invalid byte ranges, content headers and storage errors.
- [x] Implement one bounded streaming path from the private original; API validates current principal/share/file state before opening storage. No presigned GET redirects.
- [x] Stop new public copies; issue random API share tokens and use configured external API base URL. Preserve independent share expiry/revocation semantics.
- [x] Add cleanup/migration tests covering existing public copies, failed deletes and late writes; preserve durable retries and avoid reporting successful retirement prematurely.
- [x] Pass interrupted-stream tests and confirm no SQL transaction remains held during byte transfer.

**Deliverable:** API-mediated delivery with legacy cleanup and documented cutover behavior.

**SDK parity**

**Files:** crates/condite-sdk/src/lib.rs; crates/condite-sdk-node/src/; crates/condite-sdk-wasm/src/; scripts/sdk/; wire exports.

- [x] Add account/usage and applicable administration/session methods with role enforcement in core; Vencord need not expose administration UI.
- [x] Implement approved streamed/download-to-file interfaces; browser downloads use same-origin cookie endpoints where appropriate. Preserve direct upload credential separation.
- [x] Regenerate wire types and packages; test full-u64 values, partial transfers and regular-key denial of admin methods.
- [x] Run just check sdk and applicable WASM checks, serially.

**Deliverable:** all bindings expose the applicable new contracts without core dependencies.

## Task 3: Dashboard, Vencord and release validation

**Dashboard login, account page and administration**

**Files:** clients/dashboard/src/lib/{server.ts,proxy.ts}; new middleware/session helpers; src/pages/ login/logout routes; account/admin components; tests/transport.test.ts and browser tests; clients/i18n/.

- [x] Test unauthenticated SSR/proxy denial, valid/invalid login, cookie flags, CSRF, expiry, logout, rotation/removal and revoked regular keys.
- [x] Implement core-backed opaque sessions and per-request clients; remove the shared server API key model. A regular session must never inherit root credentials.
- [x] Add account usage/limits and root global/owner-filtered views; percentages use charged bytes and handle zero/unlimited limits.
- [x] Add key create/list/update/revoke with copy-once secret handling. Add server status, logging, reload, administrative file operations and confirmed shutdown.
- [x] Update proxy allowlist/byte limits for explicit routes and file streaming; preserve raw-byte JSON forwarding and safe headers.
- [x] Validate JS-disabled SSR, hydration, both locales, keyboard access and session behavior across restart/replicas. Run documented dashboard checks.

**Deliverable:** API-key dashboard accounts and root administration without manual server-key provisioning.

**Vencord usage and navigation sizing**

**Files:** clients/vencord/condite.desktop/; clients/vencord-extension/src/{navigation.tsx,styles.css,fileManager.tsx}; native/browser transport adapters; clients/i18n/; respective tests.

- [x] Display usage, total quota and per-file limits in both integrations; distinguish pending reservation from confirmed storage and handle unlimited/zero limits.
- [x] Update all public/owner download flows to the API contracts and apply approved client TOML behavior.
- [x] Match the Upload activity row to adjacent Discord navigation: outer bounds, padding, height, icon alignment, radius and hover/focus/selected states. Prefer Discord's existing navigation component where stable; avoid guessed screenshot pixel dimensions.
- [x] Verify locally in supported desktop/browser modes and report any live Discord verification unavailable. Include keyboard, text scaling and RTL checks.
- [x] Run each integration's documented tests/builds, reusing built SDK artifacts.

**Deliverable:** usage visible in both Vencord clients and a consistently sized navigation row. No Vencord key-management UI.

**Documentation and release validation**

**Files:** AGENTS.md and affected component guidance; docs/{development.md,client-capabilities.md,file-visibility-architecture.md}; component READMEs; Docker examples.

- [x] Keep capability cells factual with source references and explicit intentional exclusions; update guidance to require the matrix when adding clients/contracts/features.
- [x] Replace obsolete direct-public-bucket and server-key setup guidance; retain history as history. Document root rotation, session expiry and old-link cutover.
- [x] Check CPU/RAM/disk and run required Rust/SDK/dashboard/Vencord validation sequentially with appropriate worker limits.
- [x] Validate Docker configuration without printing resolved secrets, relative documentation links and git diff --check. Report provider/live-client checks separately from local fixtures.

**Deliverable:** verified implementation with accurate deployment instructions and a maintained parity record.
