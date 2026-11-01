# Sharing and Retention Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Track completed steps here.

**Goal:** Direct, independently revocable S3 sharing and 14-day upload retention across all clients.
**Architecture:** Private originals; durable share records publish separate public copies. Condite cleans expired copies and originals at startup and every 60 seconds.
**Tech Stack:** Rust/Axum/Toasty/S3, Node/WASM SDKs, Astro/Solid v2, Vencord React.
**Spec:** [Approved design](../specs/2026-11-01-sharing-retention-design.md).

## Global constraints

- No CloudFront, download streaming or new config.jsonc system. Document future options.
- Default retention 14 days; nullable expiry means Never; omission on creation uses default.
- Share lifecycle mutations must remain recoverable across SQL/S3 failures.
- Preserve u64/bigint, authentication boundaries, MIT/GPL licenses and existing privacy processing.
- Both Vencord integrations and dashboard cover the public user-facing HTTP API.
- Use project-local design skills, pinned Solid v2 semantics and existing just command verbs.

## Review focus

- A crash after CopyObject must leave a known key which reconciliation can delete.
- Retention extension concurrent with cleanup must not delete a retained file.
- Storage failures must not report a public copy as revoked or block unrelated cleanup.
- Private confirmation must not create public URLs; auth must never reach storage PUTs.
- Broken connection states, long filenames and RTL must preserve usable client actions.

## Contract (shared before implementers start)

- `Retention { expires_at: Option<String> }`; request `retention: Option<Retention>`.
- `Upload`: existing fields plus nullable `expires_at`, `visibility: Private|Shared`, `deletion_pending: bool`.
- `ConfirmResponse { upload: Upload, new_storage_size_bytes: u64 }`.
- `DownloadResponse { uri: String, expires_at: String }`.
- `CreateShareRequest { expires_at: Option<String>, follows_upload: bool }`.
- `Share { id, upload_id, url, created_at, expires_at: Option<String>, follows_upload: bool, status: Creating|Active|Revoking|Revoked|Expired }`.
- `ListSharesResponse { shares: Vec<Share>, next_after: Option<String> }`; list query uses `ListUploadsRequest`.
- `POST/GET /v1/uploads/{id}/shares`, `DELETE /v1/uploads/{id}/shares/{share_id}`.
- `PATCH /v1/uploads/{id}/retention` accepts Retention, returns Upload.
- `POST /v1/uploads/{id}/download` returns DownloadResponse.
- Rust SDK: `create_share(id, &CreateShareRequest)`, `list_shares(id, &ListUploadsRequest)`, `revoke_share(id, share_id)`, `update_retention(id, &Retention)`, `download(id)`; JS camelCase equivalents.

## Task 1 — Core persistence and lifecycle

Files: core models/migrations, uploads routes, runtime config, cleaner, `tests/uploads.rs` and core README/env examples.

- [x] Add failing router tests for private confirmation, multiple owner-scoped shares and nullable retention.
- [x] Run focused tests and verify missing functionality fails.
- [x] Implement contract routes, indexed models and additive migrations, including migration-time 14-day grace.
- [x] Implement durable publication/revocation/deletion and bounded 60-second cleanup with race rechecks; cap live shares at 20 per upload.
- [x] Test retry recovery, cleanup failure isolation, owner isolation, invalid/past expiry, Never, retention races and public delivery metadata.
- [x] Run core checks and document public bucket/base URL policy requirements and limitations.

## Task 2 — SDK and generated contracts (controller)

Files: shared DTOs/exporter, Rust SDK, Node/WASM bindings, SDK JS contracts, consumer example and fixtures.

- [x] Define shared DTOs before downstream work; add serialization assertions for default vs Never.
- [x] Add failing SDK fixture tests for new methods and updated confirmation.
- [x] Implement Rust/Node/WASM operations and retain retention in privacy uploads.
- [x] Regenerate types and SDK packages, update consumer/fixtures; run SDK and WASM checks.

## Task 3 — Dashboard

Files: Files/components, styles/layouts, proxy, shared i18n, dashboard fixtures/tests and README.

- [x] Add integration coverage for Share/Manage links/Expiry dialogs and owner Download.
- [x] Implement private upload default, retention picker, share management, failure feedback and list/grid parity.
- [x] Polish responsive spacing, errors, view toggle and motion using Emil skills; add AI footer notice.
- [x] Update proxy allowlist and tests; preserve raw bytes and write origin checks.
- [x] Verify check, unit/build and production/development browser tests including SSR, keyboard and locales.

## Task 4 — Both Vencord integrations

Files: `clients/vencord-extension/`, `clients/vencord/`, corresponding tests/READMEs.

- [x] Add fixtures proving API keys, retention and automatic direct share creation reach native SDK calls.
- [x] Implement all user-facing API methods via native bridges, nullable expiry and default 14-day retention.
- [x] Expose upload retention alongside privacy options; provide share/revoke/expiry management in both integrations.
- [x] Fix Home sidebar theme/hover behavior and manager layout; use author ID `955090007335530506n` and small AI about notices.
- [x] Run plugin tests, pinned upstream type/lint/build checks and document live-client limitations.

## Task 5 — Community documentation and integration review

- [x] Add AI-NOTICE, contributing/conduct/security docs, issue forms, PR template and accurate mixed-license overview.
- [x] Use GitHub-only contact channels; enable private vulnerability reporting if permitted.
- [x] Replace proposal docs with implemented behavior, preserve streaming/config.jsonc future notes.
- [x] Review the entire change independently, fix findings, run appropriate workspace/client checks and diff checks.
- [x] Audit every approved requirement; deliver tiny GitHub description in a code block.

## Execution ledger

- Approved design: `5a557ad`. Existing isolated worktree on `feat/private-sharing-links`.
- Ruling: execute now under existing implementation approval; do not introduce a second plan-approval interruption.
- Ruling: flat nullable expiration DTOs avoid sentinel dates and preserve omission-vs-Never without JSON integer conversions.

- Final validation: workspace Rust/all-features, WASM, SDK/consumer/runtime suites,
  dashboard check/build/unit12/prod17/dev17, Vencord privacy15/extension21 tests
  and both strict upstream builds passed. Independent review findings addressed.
- Recovery refinement: opaque private-key tombstones retry daily after initial
  cleanup; ambiguous public copies remain in minute reconciliation with hidden
  URLs and HTTP202 pending revocation. No public download gateway was added.
- External verification limits: real S3 policy/consistency and authenticated
  Discord behavior require deployment/live-client checks; see component READMEs.
