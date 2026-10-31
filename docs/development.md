# Development

## Where changes belong

| Area | Source | Local guidance |
| --- | --- | --- |
| HTTP, SQL, S3, shutdown | `crates/condite-core/` | [Core README](../crates/condite-core/README.md), [agent guidance](../crates/condite-core/AGENTS.md) |
| API wire types and TypeScript export | `crates/condite-shared/` | [Shared README](../crates/condite-shared/README.md) |
| HTTP client and upload orchestration | `crates/condite-sdk/` | [Rust SDK](../crates/condite-sdk/README.md) |
| Native and browser bindings | `crates/condite-sdk-node/`, `crates/condite-sdk-wasm/` | [SDK tooling](../scripts/sdk/README.md) |
| SSR, browser UI and API proxy | `clients/dashboard/` | [Dashboard README](../clients/dashboard/README.md), [agent guidance](../clients/dashboard/AGENTS.md) |

The Rust workspace shares a root lockfile and `target/`. The SDK build uses
`target/sdk` and generates ignored platform packages under the binding crates'
`dist/` directories. Build those packages **before** installing the dashboard's
local file dependencies. Use npm with the root lockfile and Bun with the dashboard
lockfile; these are separate dependency installations.

## Validation

Run commands from the repository root unless specified:

| Change | Checks |
| --- | --- |
| Rust service or shared/client code | `just check-rust` (format, Clippy, tests; all targets/features) |
| Browser Rust | `just check-sdk-wasm` (requires the WASM target) |
| SDK bindings or generated types | `just check-sdk` (build, independent consumer, lint/types, runtime tests, WASM Clippy) |
| Dashboard | In `clients/dashboard`: `bun run check`, `bun run test:unit`, `bun run build`, `bun run test`, `bun run test:dev` |
| Documentation only | Verify relative links and source claims; run `git diff --check` |

See [SDK prerequisites](../scripts/sdk/README.md) before running SDK checks.
Core integration tests use an in-memory database and fake S3; SDK runtime tests
use local HTTP fixtures. Dashboard browser tests use real built SDKs with a local
fixture, not a live database/bucket. These do not prove a provider's real S3/CORS
configuration; verify that separately for deployment.

CI currently has Rust and SDK workflows, described in [CI details](ci.md).
Dashboard checks are a separate local requirement; neither workflow runs them.

## Contract and schema changes

Update wire types in `condite-shared`, adjust handlers/client/bindings together,
then rebuild SDKs and check the independent consumer. Preserve `u64` precision:
HTTP uses JSON integers while JS SDKs expose bigint. The dashboard proxy forwards
raw bytes instead of parsing and serializing those integers.

To change SQL models, run `just migrate <name>`. This generates migration files;
startup applies embedded migrations when opening the database. Review and commit
SQL, snapshots and `toasty/history.toml` together. Back up existing data before
running a new build against a persistent database.

## Documentation maintenance

Keep component setup and operational behavior in the component README. Put shared
workflows here, CI rationale in `ci.md`, and future sharing requirements in
[file visibility architecture](file-visibility-architecture.md). Historical specs
and plans under `superpowers/` explain intent; verify current behavior in source.
Record limitations explicitly rather than describing planned features as shipped.
