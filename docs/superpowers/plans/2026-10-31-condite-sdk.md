# Condite SDK Implementation Plan

> Use superpowers:executing-plans to implement task by task.

**Goal:** Independent Rust SDK with Electron and browser bindings and measured release sizes.
**Architecture:** Shared Serde wire types; reqwest transport; separate napi-rs and wasm-bindgen adapters.
**Tech Stack:** Rust, Serde, reqwest, ts-rs, napi-rs, wasm-bindgen.
**Spec:** ../specs/2026-10-31-condite-sdk-design.md

## Constraints and review focus

No SDK dependency on core/AWS/Toasty/Axum. Preserve HTTP wire format. Check deployment prefixes, encoded IDs, empty deletes, storage-header isolation, partial uploads, and lossless u64 JS conversion. Keep generation and platform dependencies out of ordinary consumers. Keep documentation concise.

## Tasks

- [x] Shared contract: add crates/condite-shared with public HealthStatus, PresignRequest/Response, ListUploadsRequest/Response, Upload, UploadStatus, ConfirmResponse, UploadResult; Serde round-trip fixtures before implementation; ts-rs declaration generator. Migrate core handlers to these types and map database objects explicitly. Run shared and existing core tests.
- [x] Rust client: add crates/condite-sdk. Integration tests first for health, presign/PUT/confirm, paginated listing, deletion, error bodies, prefix/ID escaping and isolated headers. Implement Client::new/with_client, health, presign, list_uploads, confirm, delete, put_upload, upload. Preserve request IDs on partial failures. Run native tests and standalone example.
- [x] Bindings: add separate crates/condite-sdk-node and crates/condite-sdk-wasm with equivalent Promise APIs, bigint fields, typed declarations, error details, and byte inputs. Write shared JS HTTP-fixture tests before implementation; run them in Node, Electron, and browser WASM. Build scripts and generated declarations belong with adapters.
- [x] Delivery: add SDK-specific release profile, reproducible check/build commands, concise consumption/CORS documentation and artifact size report. Verify workspace formatting/clippy/tests, declaration drift, target isolation and standalone consumption; request whole-change review and address material findings.

## Execution ledger

- Libraries and design approved by user. Existing linked worktree retained. Execute inline under that authorization; no additional plan approval gate.

- Implemented shared contract and core conversions, independent reqwest client, and separate Node/WASM adapters. MIT applied to all new crates; core license unchanged.
- Verified: workspace all-target/all-feature tests; native workspace Clippy with warnings denied; WASM-target Clippy; formatting; Prettier/ESLint/TypeScript; independent consumer; normal dependency isolation; real Node, Electron main process, and Chromium WASM HTTP contracts.
- Review: exposed upload stage consistently in JS errors; regression contracts cover PUT and confirmation failures and out-of-range bigint input.
- Size snapshot: docs/sdk-sizes.json. Browser WASM payload 431787 bytes, 164009 gzip; native addon 2889968 bytes on arm64 Linux. Core release settings unchanged.
