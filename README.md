# Condite

Self-hosted file storage backed by your own S3-compatible bucket. The current
project includes a Rust HTTP service, Rust/Node/browser SDKs, and an SSR web
console for listing, uploading, confirming and deleting files.

This is early software: the API has no authentication or owner isolation.
Deploy the API and dashboard behind trusted access. Download links currently
expire; public/private visibility is [planned](docs/file-visibility-architecture.md).
The Vencord upload extension is also planned.

## Start here

- [Core setup and API](crates/condite-core/README.md): database, bucket and server.
- [Dashboard setup](frontends/dashboard/README.md): SDK build, development and deployment.
- [Development guide](docs/development.md): repository map, checks and change workflow.
- [SDK build and tests](scripts/sdk/README.md): native addon and browser WASM prerequisites.
- SDK usage: [Rust](crates/condite-sdk/README.md),
  [Node/Electron](crates/condite-sdk-node/README.md),
  [browser](crates/condite-sdk-wasm/README.md).
- [CI details](docs/ci.md) and [sharing proposal](docs/file-visibility-architecture.md).

## Local development

Install [Just](https://just.systems), Rust/rustup, Node (dashboard: 22.23.4+),
Bun 1.4.4+, and a C toolchain. Then, from the repository root:

```sh
just setup                # Backend + SDK tooling/build + dashboard dependencies
# Edit crates/condite-core/.env with your database and S3 credentials.
just doctor               # Check all prerequisites and configuration
just backend              # Terminal 1: API on :8080
just dashboard            # Terminal 2: dashboard on :4321 (open /en/ or /es/)
```

Use `just install <backend|dashboard|sdk|all>` to install dependencies and
`just build <backend|dashboard|sdk|all>` to build. `just setup` is a shortcut for
`just install all`. Building all regenerates SDKs and builds both applications.
Dashboard installation prepares the SDKs first; dashboard builds assume
installation is complete.
`just build sdk` regenerates shared types,
Node/WASM packages and the Rust example. Re-run it after SDK
changes; workspace links expose the regenerated packages to the dashboard.

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

`just doctor backend`, `just doctor dashboard`, and `just doctor sdk` narrow the
checks. Doctor is read-only, returns nonzero for missing prerequisites, and does
not probe the database or S3. SDK browser/Electron test dependencies are documented
in [SDK development](scripts/sdk/README.md).

Local justfiles in `crates/condite-core`, `frontends/dashboard`, and `scripts/sdk`
provide `install`, `build`, `doctor`, and `run` (apps). Install commands
preserve existing `.env` files. Run `just` anywhere for the relevant command list.

## Run the service

Install stable Rust and [Just](https://just.systems), provision an S3-compatible
bucket, then:

```sh
cp crates/condite-core/.env.example crates/condite-core/.env
# Edit .env with your database URL and real bucket credentials.
just run
```

The example uses a local Turso database. Startup applies embedded migrations and
binds to `0.0.0.0:8080`. `GET /v1/health` reports process health; it does not probe
storage. See the core README for storage permissions and browser CORS requirements.

## Repository

Rust packages live under `crates/`; the Cargo workspace defaults to `condite-core`.
Use `--workspace` to check all packages. The dashboard lives in
`frontends/dashboard/`; root Bun commands build the SDKs and dashboard.
The independent Rust consumer in `examples/sdk-consumer/` has its own workspace
and lockfile. Core is AGPL-3.0-or-later; shared types and SDK crates are MIT.
