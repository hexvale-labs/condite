# Condite

Self-hosted file storage backed by your own S3-compatible bucket. The current
project includes a Rust HTTP service, Rust/Node/browser SDKs, and an SSR web
console for listing, uploading, confirming and deleting files.

This is early software: the API has no authentication or owner isolation.
Deploy the API and dashboard behind trusted access. Download links currently
expire; public/private visibility is [planned](docs/file-visibility-architecture.md).
A [desktop Vencord userplugin](clients/vencord/README.md) adds Condite uploads and
local privacy options beside the Discord message box. Installation requires a
source-built Vencord; it is not enabled automatically.

## Start here

- [Core setup and API](crates/condite-core/README.md): database, bucket and server.
- [Dashboard setup](clients/dashboard/README.md): SDK build, development and deployment.
- [File privacy coverage](crates/condite-metadata/README.md): metadata, filenames, encryption and limits.
- [Development guide](docs/development.md): repository map, checks and change workflow.
- [SDK build and tests](scripts/sdk/README.md): native addon and browser WASM prerequisites.
- SDK usage: [Rust](crates/condite-sdk/README.md),
  [Node/Electron](crates/condite-sdk-node/README.md),
  [browser](crates/condite-sdk-wasm/README.md).
- [CI details](docs/ci.md) and [sharing architecture](docs/file-visibility-architecture.md).

## Local development

Install [Just](https://just.systems), Rust/rustup, Node (dashboard: 22.23.4+),
Bun 1.4.4+, and a C toolchain. Then, from the repository root:

```sh
just install all                # Backend + SDK tooling/build + dashboard dependencies
# Edit crates/condite-core/.env with your database and S3 credentials.
just check tools               # Check all prerequisites and configuration
just run backend              # Terminal 1: API on :8080
just run dashboard            # Terminal 2: dashboard on :4321 (open /en/ or /es/)
```

Run `just help` for all commands and examples. `just install all` includes the
Vencord toolchain setup and mprocs (requires pnpm for Vencord). Use `just run all`
to manage the backend and dashboard together in [mprocs](https://github.com/Rundfunk/mprocs),
or run each component separately. Database and S3 services must be provisioned
separately. `just build all` includes SDKs, backend, dashboard and Vencord.
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

`just check tools backend`, `just check tools dashboard`, and `just check tools sdk` narrow the
checks. Tool checking is read-only, returns nonzero for missing prerequisites, and does
not probe the database or S3. SDK browser/Electron test dependencies are documented
in [SDK development](scripts/sdk/README.md).

Local justfiles in `crates/condite-core`, `clients/dashboard`, and `scripts/sdk`
provide `install`, `build`, `check`, `help`, and `run` (apps). Install commands
preserve existing `.env` files. Run `just` anywhere for the command guide.

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
`clients/dashboard/`; root Bun commands build the SDKs and dashboard.
The independent Rust consumer in `examples/sdk-consumer/` has its own workspace
and lockfile. Core is AGPL-3.0-or-later; shared types and SDK crates are MIT.

## Contributing and transparency

See [contributing](CONTRIBUTING.md), [conduct](CODE_OF_CONDUCT.md),
[security reporting](SECURITY.md), and the [license overview](LICENSE).
This project was [written with AI assistance](AI-NOTICE.md).
