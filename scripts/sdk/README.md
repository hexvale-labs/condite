# SDK development

Prerequisites: Rust, Node 22+, a C toolchain, `wasm32-unknown-unknown`, and wasm-bindgen CLI matching Cargo.lock (currently 0.2.129).

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
npm ci
npx playwright install --with-deps chromium
npm run build:sdk
npm run check:consumer
npm run lint:sdk
npm run test:sdk
just check-rust
just check-sdk-wasm
```

On headless Linux the Electron main-process test uses `xvfb-run`; Electron also needs GTK 3 runtime libraries. Set `WASM_BINDGEN` to an alternate CLI path if needed. The build uses `target/sdk` and the SDK-only size profile; core's release configuration is unchanged.

Builds produce `crates/condite-sdk-{node,wasm}/dist` packages and `target/sdk-sizes.json` with raw addon, WASM, gzip WASM, glue, and minimal Rust-example sizes. The checked-in measurement snapshot is [docs/sdk-sizes.json](../../docs/sdk-sizes.json). These depend on platform and toolchain; they are measurements, not a cross-platform size guarantee. Generation runs ts-rs from shared wire types and supplements it with the public class signatures. TypeScript checks exercise both packages.

The runtime suite loads the actual addon in Node and Electron and the compiled WASM in Chromium, using local HTTP fixtures for all five API methods, upload flow, bigint precision, and partial failures. Core integration tests also run the Rust SDK against the actual server router. The independent `examples/sdk-consumer` project checks embedding outside this workspace. All SDK/shared/wrapper crates are MIT; condite-core retains AGPL-3.0-or-later.
