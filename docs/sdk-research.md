# SDK research

Status: libraries and design approved; implementation and verification tracked in the SDK plan.

## API surface

- `GET /v1/health`: health, start timestamp, runtime span.
- `POST /v1/uploads`: size, MIME type, filename → signed PUT URL, headers, expiry, request ID.
- `GET /v1/uploads`: cursor pagination (`limit` 1–100, `after`) → uploads and next cursor.
- `POST /v1/uploads/{id}/confirm`: signed download URL, expiry, total storage size.
- `DELETE /v1/uploads/{id}`: successful empty 204 response.

The separate Unix control socket is not browser accessible. Keep it outside the initial HTTP SDK scope unless requested.

## Proposed boundary

`condite-sdk` must not depend on `condite-core`, AWS, Toasty, or Axum.
Consider a small shared wire-types crate consumed by core and SDK to prevent drift.
Keep Node and browser adapters optional or in separate crates so Rust consumers do not build them.
Provide individual operations plus a presign → PUT → confirm helper, cursor primitives, and errors preserving HTTP status and response details.

## Approved libraries and considered alternatives

- [reqwest](https://docs.rs/reqwest/latest/reqwest/): requested HTTP client; disable defaults and select JSON, query, and native TLS features deliberately. Browser WASM uses browser networking, with different timeout and TLS configuration support.
- [ts-rs](https://docs.rs/ts-rs/latest/ts_rs/): generate TypeScript wire types from Rust, enabled only for generation.
- [wasm-bindgen](https://wasm-bindgen.github.io/wasm-bindgen/): callable browser bindings.
- [napi-rs](https://napi.rs/): callable Electron Node-process bindings.
- Alternative: [utoipa](https://docs.rs/utoipa/latest/utoipa/) generates OpenAPI from server types and handlers; [typify](https://docs.rs/typify/latest/typify/) generates Rust types from JSON Schema, not a complete HTTP client.

## Compatibility checks

- Core router currently has no CORS middleware. Cross-origin browser calls and direct signed S3 PUTs require suitable API and bucket CORS policies.
- Core serializes `u64` fields as JSON numbers; ts-rs defaults large integers to `bigint`. Define and test JS conversions, including values above the safe integer range.
- Keep UUIDs and timestamps compatible with current wire serialization; health runtime is a serialized Jiff span.
- Signed PUT headers can include headers managed by the browser. Verify an actual browser upload rather than assuming every header can be set.
- Measure release WASM and native addon sizes with recorded build flags; dependency count alone does not prove small artifacts.
- Verify a standalone Rust consumer, a real Electron process, and browser HTTP/upload behavior.

Research environment: Node v22.22.1; installed Rust target is aarch64-unknown-linux-gnu. Browser WASM target and Electron runtime will be needed for verification.
