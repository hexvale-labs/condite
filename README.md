# Condite

Condite is planned as self-hosted file storage with a web panel for managing files and their lifetimes. You bring your own S3-compatible storage credentials; Condite uses your bucket to store and serve files.

The project will also support extensions that connect other apps to your storage. The first planned extension is for Vencord: it will intercept Discord's file upload handler and route uploads to Condite.

The current codebase is an early Rust service with S3 upload URL generation. See [`crates/condite-core/README.md`](crates/condite-core/README.md) for setup and API details.

## Repository layout

- `Cargo.toml` and `Cargo.lock`: Rust workspace and shared dependency lockfile.
- `crates/condite-core/`: service library, server and migration binaries, integration
  tests, and Toasty migrations.
- `justfile`: development commands; run `just run`, `just migrate <name>`, or
  `just check-rust` from the repository root.

Rust packages live under `crates/`, following the
[Cargo workspace convention](https://doc.rust-lang.org/cargo/reference/workspaces.html).
Each package keeps the standard `src/`, `src/bin/`, and `tests/` layout.
Add future Rust packages under `crates/`; add the planned web panel and extensions
when they are implemented.

Run `cargo check --locked --workspace` from the repository root. Build artifacts
are shared in the root `target/` directory. The run and migration recipes execute
inside the service directory so its `.env` and `Toasty.toml` remain local to it.

## Dashboard

The SSR admin console lives in [`frontends/dashboard`](frontends/dashboard/README.md).
It uses Astro, SolidJS v2 RC, Tailwind and the generated SDKs, with Bun for
development and Node in production. Public/private file delivery is planned in
[the visibility proposal](docs/file-visibility-architecture.md); no public domain
is chosen yet.

## Shared dependencies

Run `bun install --frozen-lockfile` at the root to install all JavaScript
workspaces from one `bun.lock`. Shared tooling versions live in the root catalog;
Solid integration pins and overrides also stay centralized. SDK workspace links
resolve before generation: run `bun run build:sdk` before using the dashboard.
`bun run build` generates the SDKs and builds the dashboard in order.
Use `bun run dev:dashboard`, `bun run check:dashboard`, and
`bun run build:dashboard` from the root. Electron's install script is trusted
explicitly; production SSR still runs on Node.

Rust dependency versions live in `[workspace.dependencies]`; crates opt into
features locally to keep native and browser dependencies isolated. SDK development
builds share `target/` with the service. `bun run build:sdk:release` retains the
size-optimized release build. The external Rust consumer deliberately keeps its
own workspace and lockfile to test independent use.
