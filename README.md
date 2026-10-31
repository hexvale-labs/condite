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
`frontends/dashboard/`; root npm commands build and test the SDKs only.
The independent Rust consumer in `examples/sdk-consumer/` has its own workspace
and lockfile. Core is AGPL-3.0-or-later; shared types and SDK crates are MIT.
