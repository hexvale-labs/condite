# Condite SDK design

## Intent

Provide a small, reusable `condite-sdk` for native Rust, Electron's Node process, and browser WASM. The SDK uses reqwest and never depends on condite-core. The user approved ts-rs, wasm-bindgen, and napi-rs.

## Approach

Use shared Rust wire types and generate TypeScript declarations with ts-rs. This avoids duplicating the API contract. The alternative was OpenAPI generation with utoipa and schema-driven client generation; its extra setup is unnecessary for the current five HTTP operations. Keep handwritten HTTP methods small and explicit.

## Crates

- `condite-shared`: Serde request/response types and upload status, without HTTP, database, or server dependencies. Generation-only ts-rs feature. Represent UUIDs and timestamp/span wire values as strings to avoid UUID generation and timezone machinery in consumers. Keep integer fields as u64 in Rust.
- `condite-sdk`: async reqwest client, reusable request and pagination types reexported from shared, transport/HTTP/decode/input errors, and upload primitives. Reqwest defaults disabled; explicitly enable JSON/query support and optional native TLS. Accept an existing reqwest client for host configuration. No embedded native runtime; Rust hosts supply Tokio.
- `condite-sdk-node`: optional separate napi-rs addon wrapping the SDK for Electron main/utility Node processes; async Promise methods and generated declarations. Normal Rust consumers do not build it.
- `condite-sdk-wasm`: optional separate wasm-bindgen adapter for wasm32-unknown-unknown, async Promise methods and generated declarations. Browser networking uses reqwest's WASM backend. No WASI requirement.

Core consumes shared request/response types, converts database records into wire records, and retains its own Toasty upload-status type with explicit conversion. Existing JSON field names, enum capitalization, timestamps, pagination, and status codes remain compatible. Control socket APIs remain separate because browsers cannot use Unix sockets.

## Public operations

Health, presign, list a page, confirm, and delete map directly to current routes. Expose signed PUT as its own reusable operation and provide a presign → PUT → confirm helper returning the confirmation and request ID. Byte buffers work on every target; streaming/file-system helpers can follow later. Do not silently retry writes or delete an upload after partial failure; retain request IDs so callers can retry confirmation or clean up.

Base URLs must support deployment path prefixes. Encode path parameters, serialize query parameters, preserve non-success HTTP status/body, and accept successful empty DELETE responses. Keep API authorization separate from signed object-storage requests; do not forward API headers to storage. Browser-controlled signed headers such as Host and Content-Length are left to the browser.

## JavaScript contract

Expose structured objects and Uint8Array/Buffer upload bytes. JavaScript u64 fields use bigint in both adapters; explicit boundary conversion preserves integers rather than routing responses through JSON.parse. ts-rs declarations must match these runtime values. IDs, timestamps, and runtime spans remain strings. Binding errors expose stable operation/error information without requiring users to parse Rust debug output.

## Size and validation

Keep generation dependencies feature-gated, native bindings separate, and native TLS target-specific. Record optimized WASM, compressed WASM, native addon, and a minimal Rust-consumer size with toolchain/build flags; report measurements without inventing a size budget. Add reproducible build instructions and focused integration checks for API wire compatibility, queries, status/errors, complete upload flow, and precision at JS boundaries.

Verify standalone Rust consumption and dependency isolation, load the Node addon in Electron, and exercise compiled WASM in a browser. Use local HTTP fixtures for deterministic client tests; use existing core integration tests to catch server regressions. Browser cross-origin deployments require API and S3 CORS configuration; document this prerequisite rather than widening the core's access policy automatically.

## Builder decisions

Choose exact compatible library versions from current docs, ergonomic method names, test tooling, release flags scoped to SDK artifacts, and packaging scripts. License shared, SDK, and wrappers under MIT as requested; retain core's existing AGPL license. Do not publish packages or alter deployment configuration.
