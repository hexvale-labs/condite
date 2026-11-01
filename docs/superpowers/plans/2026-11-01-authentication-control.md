# Authentication and control CLI implementation plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deliver authenticated uploads with per-key quotas and a Unix-socket CLI.
**Architecture:** Shared control DTOs, database key ownership and transactional row locking, isolated CLI crate.
**Tech stack:** Rust, Axum, Toasty, Clap, getrandom, SHA-256.
**Spec:** ../specs/2026-10-31-authentication-control-design.md

## Global constraints

- Defaults: 100 MiB per upload, 1 GiB total; suffixes mandatory and binary.
- Pending records reserve storage; revoked keys cannot authenticate.
- Keep AWS/database dependencies out of shared and CLI crates.
- Preserve S3/SQL retry recovery and require an owner for every upload after the requested database reset.

## Review focus

- Concurrent reservations and limit changes must serialize in the database.
- Cross-owner IDs must return 404 without touching storage.
- Partial S3 failures retain metadata and reserved bytes.
- Socket framing must be bounded and errors must reach the CLI.
- API credentials must never reach S3 or browser bundles accidentally.

### Task 1: Key persistence, authentication, ownership and quotas

Files: core db/sql/api_key.rs, auth.rs, upload handlers, cleaner, migrations, tests/uploads.rs.
Produces: key generation/update/revocation helpers, authenticated request extractor, transaction locking helper.
- [x] Add failing router tests for missing/invalid credentials, ownership and quota boundaries.
- [x] Add key model and required upload owner (after the requested reset); generate migration using `just migrate`.
- [x] Authenticate Bearer tokens by digest; serialize mutations using a key-row write inside transactions.
- [x] Scope upload operations and totals; reserve pending bytes; preserve cleanup recovery.
- [x] Run core tests including concurrent requests and quota changes.

### Task 2: Control protocol and CLI

Files: shared control.rs, core control modules, crates/condite-ctl/.
Consumes: Task 1 key and upload helpers. Produces: generate/update/delete commands and executable.
- [x] Add failing size-parser and socket CLI integration tests.
- [x] Move protocol DTOs to shared; add structured errors and bounded framing.
- [x] Implement Clap commands, defaults, suffix parsing, discovery, timeout and JSON output.
- [x] Restrict socket permissions; test real socket round trips and command failures.

### Task 3: Consumers, documentation and verification

Files: dashboard proxy/server configuration, SDK docs and authentication support as needed, component READMEs.
- [x] Add tests for credential forwarding without S3 leakage.
- [x] Configure server-side dashboard credentials; document SDK headers and CLI usage.
- [x] Run `just check-rust`, affected SDK/dashboard checks, and `git diff --check`.
- [x] Review the complete change and resolve correctness/security findings.

Execution: inline in the existing isolated worktree, as requested by “carry on”.
