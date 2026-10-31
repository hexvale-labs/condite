# Condite

Condite is planned as self-hosted file storage with a web panel for managing files and their lifetimes. You bring your own S3-compatible storage credentials; Condite uses your bucket to store and serve files.

The project will also support extensions that connect other apps to your storage. The first planned extension is for Vencord: it will intercept Discord's file upload handler and route uploads to Condite.

The current codebase is an early Rust service with S3 upload URL generation. See [`crates/condite-core/README.md`](crates/condite-core/README.md) for setup and API details.

## Local development

Install [Just](https://just.systems), Rust/rustup, Node (dashboard: 22.23.4+),
npm, Bun 1.4.4+, and a C toolchain. Then, from the repository root:

```sh
just setup                # Backend + SDK tooling/build + dashboard dependencies
# Edit crates/condite-core/.env with your database and S3 credentials.
just doctor               # Check all prerequisites and configuration
just backend              # Terminal 1: API on :8080
just dashboard            # Terminal 2: dashboard on :4321 (open /en/ or /es/)
```

Use `just install <backend|dashboard|sdk|all>` to install dependencies and
`just build <backend|dashboard|sdk|all>` to build. `just setup` is a shortcut for
`just install all`. Building all regenerates SDKs, refreshes the dashboard local
SDK copies, and builds both applications. Dashboard installation prepares
the SDKs first; dashboard builds assume installation is complete.
`just build sdk` regenerates shared types,
Node/WASM packages, the Rust example, and the size report. Re-run it after SDK
changes, then run `bun install --frozen-lockfile` in `frontends/dashboard` to
refresh the dashboard's local package copies.

`just doctor backend`, `just doctor dashboard`, and `just doctor sdk` narrow the
checks. Doctor is read-only, returns nonzero for missing prerequisites, and does
not probe the database or S3. SDK browser/Electron test dependencies are documented
in [SDK development](scripts/sdk/README.md).

Local justfiles in `crates/condite-core`, `frontends/dashboard`, and `scripts/sdk`
provide `install`, `build`, `doctor`, and `run` (apps). Install commands
preserve existing `.env` files. Run `just` anywhere for the relevant command list.

## Repository layout

- `Cargo.toml` and `Cargo.lock`: Rust workspace and shared dependency lockfile.
- `crates/condite-core/`: service library, server and migration binaries, integration
  tests, and Toasty migrations.
- `justfile`: development commands; run `just run`, `just migrate <name>`, or
  `just check-rust` from the repository root.

Rust packages live under `crates/`, following the
[Cargo workspace convention](https://doc.rust-lang.org/cargo/reference/workspaces.html).
Each package keeps the standard `src/`, `src/bin/`, and `tests/` layout.
Add future Rust packages under `crates/`.

Run `cargo check --locked --workspace` from the repository root. Build artifacts
are shared in the root `target/` directory. The run and migration recipes execute
inside the service directory so its `.env` and `Toasty.toml` remain local to it.

## Dashboard

The SSR admin console lives in [`frontends/dashboard`](frontends/dashboard/README.md).
It uses Astro, SolidJS v2 RC, Tailwind and the generated SDKs, with Bun for
development and Node in production. Public/private file delivery is planned in
[the visibility proposal](docs/file-visibility-architecture.md); no public domain
is chosen yet.
