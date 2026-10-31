# CI performance

Both workflows run on pull requests, pushes to `main`, and manual dispatches.
Feature branches without a PR can use manual dispatch. This avoids running the
same checks for both a feature push and its PR. Check names and coverage stay the
same. Ubuntu is pinned to 24.04 to keep native build dependencies consistent.

## Cache findings

The [audited SDK run](https://github.com/hexvale-labs/condite/actions/runs/41223368715)
had an exact Rust cache hit but rebuilt the independent consumer's dependencies.
Its `examples/sdk-consumer/target` directory was outside the cached root target.
The SDK cache now registers both workspaces, including both lockfiles in its key.
SDK builds now share the root `target` directory with other workspace commands.

[Swatinem/rust-cache](https://github.com/Swatinem/rust-cache/tree/v2.9.3)
caches dependencies and removes workspace artifacts and incremental state.
Seeing Condite crates compile after a hit is expected. Rust and SDK jobs keep
separate caches because their profiles, targets, and features differ. The root
`Cargo.toml` is explicitly keyed because this virtual workspace defines the SDK
profile outside the member manifests hashed by the action.

Compiler or build-environment changes invalidate the cache. Floating stable Rust
still intentionally follows stable releases. Cache entries are immutable: an
exact hit is restored, not rewritten. New paths or keys need an initial warm-up.

## Tool setup

A [cold SDK run](https://github.com/hexvale-labs/condite/actions/runs/41218338609)
spent 1m43s compiling wasm-bindgen-cli. Warm runs already reused that binary.
CI now downloads a checksum-verified prebuilt CLI using
[install-action](https://github.com/taiki-e/install-action/tree/v2.88.1), with its
version read from `Cargo.lock` and source compilation fallback disabled. Cargo
binary caching is disabled to avoid duplicating tool installation state.

Playwright installs only Chromium's headless shell, matching `chromium.launch()`
in the integration tests. Browser caching is omitted per
[Playwright's guidance](https://playwright.dev/docs/ci#caching-browsers); Linux
system dependencies still require installation. JavaScript tooling uses the shared Bun workspace and `bun install --frozen-lockfile`.
The SDK job explicitly runs `build:sdk:release` for size measurements.

`setup-node` and `upload-artifact` use current releases. Checkout, rust-cache,
and setup-just were already current. Dependabot already groups monthly action
updates. Keep Node 22, the SDK size profile, and the Cargo concurrency limit;
changing runtime coverage or native compiler memory use needs separate evidence.
