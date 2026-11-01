# CI performance

Both workflows run on pull requests, pushes to `main`, and manual dispatches.
Feature branches without a PR can use manual dispatch. This avoids running the
same checks for both a feature push and its PR. Check names and coverage stay the
same. Ubuntu is pinned to 24.04 to keep native build dependencies consistent.

## Cargo caches

Both jobs use the [local Cargo cache action](../.github/actions/cargo-cache/action.yml).
Its fallback prefix includes the job kind, OS/architecture, Rust compiler and Cargo,
native compiler/linker, OpenSSL version, and Rust/Cargo build environment. The exact
key additionally hashes manifests (including the virtual root), lockfiles, Cargo
configuration, toolchain files, SDK scripts/package commands, and workflow/cache
definitions.
Dependency changes can therefore restore compatible artifacts; Cargo validates
fingerprints and rebuilds changed inputs. Rust and SDK entries remain separate.

The cache contains Cargo registry/git data and complete target directories,
including `examples/sdk-consumer/target`. Incremental compilation is disabled.
Workspace artifacts are retained; Cargo may still rebuild them after checkout
because source timestamps change. Entries save only after the entire job succeeds.
Failed/cancelled jobs cannot seed immutable partial entries. Coverage changes in
workflows or build scripts create a new exact key. Bump `cargo-v1` in the local
action to retire an incomplete or incompatible cache generation.

The previous root-manifest hash was in the fallback prefix, causing complete
misses whenever `Cargo.toml` changed. A cancelled Rust run also saved partial test
dependencies, which subsequent exact hits could not update. The new generation
starts cold once and avoids both failure modes.

GitHub scopes PR caches to the PR merge ref: `main` cannot reuse them and must warm
its own entry after merge. Full artifacts use more space than dependency-only
caches; GitHub can evict older entries under repository limits. The job summary
distinguishes exact hits, compatible fallback restores, and cold starts, and lists
the restored target directory sizes.

Cold runs saved complete [Rust](https://github.com/hexvale-labs/condite/actions/runs/41287586208/attempts/1)
and [SDK](https://github.com/hexvale-labs/condite/actions/runs/41287585871/attempts/1)
entries. The [SDK warm rerun](https://github.com/hexvale-labs/condite/actions/runs/41287585871/attempts/2)
and [Rust warm rerun](https://github.com/hexvale-labs/condite/actions/runs/41287586208/attempts/2)
restored exact hits and rebuilt only workspace crates, with no registry
dependency compilation.

## Tool setup

A [cold SDK run](https://github.com/hexvale-labs/condite/actions/runs/41218338609)
spent 1m43s compiling wasm-bindgen-cli. Warm runs already reused that binary.
CI now downloads a checksum-verified prebuilt CLI using
[install-action](https://github.com/taiki-e/install-action/tree/v2.88.1), with its
version read from `Cargo.lock` and source compilation fallback disabled. The Cargo cache excludes installed CLI binaries.

Playwright installs only Chromium's headless shell, matching `chromium.launch()`
in the integration tests. Browser caching is omitted per
[Playwright's guidance](https://playwright.dev/docs/ci#caching-browsers); Linux
system dependencies still require installation. JavaScript tooling uses the shared Bun workspace and `bun install --frozen-lockfile`.
The SDK job explicitly runs `build:sdk:release` for size measurements.

`setup-node` and `upload-artifact` use current releases. Checkout, rust-cache,
and setup-just were already current. Dependabot already groups monthly action
updates. Keep Node 22, the SDK size profile, and the Cargo concurrency limit;
changing runtime coverage or native compiler memory use needs separate evidence.
