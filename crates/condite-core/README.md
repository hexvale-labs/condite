# condite core

The Rust service starts with `just run` from the repository root or `cargo run`
from this directory. The binary loads
`.env` when present, then initializes application state before binding to
`0.0.0.0:8080`.

## Local setup

From the repository root:

```sh
cp crates/condite-core/.env.example crates/condite-core/.env
# Replace the example S3 credentials and endpoint before starting.
just run
```

The example `SQL_CONN_URL=turso:./condite.db` creates a local database relative to
core's working directory. Opening the database applies embedded Toasty migrations
automatically; there is no separate initial migration command. Back up persistent
data before deploying schema changes. `just migrate <name>` generates migrations
for development; commit the SQL, snapshot and history together.

Required environment variables:

| Variable | Purpose |
| --- | --- |
| `SQL_CONN_URL` | Database connection URL passed to Toasty |
| `S3_ENDPOINT` | S3-compatible service endpoint |
| `S3_REGION` | S3 signing region |
| `S3_BUCKET` | Bucket used for uploads |
| `S3_ACCESS_KEY` | S3 access key |
| `S3_SECRET_KEY` | S3 secret key |

Startup opens the database connection and initializes the S3 client concurrently.
S3 initialization does not probe the bucket or require list permission. Credentials
need permission to put objects under `pending/` when clients use upload URLs.
Missing environment variables or database connection failures stop startup.

## Source layout

- `main.rs`: binary entry point and optional `.env` loading.
- `lib.rs`: application startup, listener, and currently registered routes.
- `state/app.rs`: active runtime configuration (`AppRuntimeConfig::load_sync`),
  shared application state and connection initialization.
- `state/config.rs`: separate configuration implementation, currently not used by
  startup (it remains exported by `state/mod.rs`).
- `state/env.rs`: environment-variable names and required-value lookup.
- `db/`: database and S3 connection setup.
- `v1/`: API modules, including upload URL generation.
- `control/`: command bus and Unix control socket.
- `toasty/`: embedded SQL migrations, snapshots and history.
- `tests/uploads.rs`: router and Rust SDK integration tests with fake S3.

## Health and runtime

`GET /v1/health` returns `healthy`, `started_at` and `runtime`. It reports process
state, not a live database or bucket readiness probe. HTTP binds to
`0.0.0.0:8080`; the port is currently fixed.

The local control socket is `$XDG_RUNTIME_DIR/condite/condite.sock`, falling back
to `/run/condite/condite.sock`. Use a writable, private runtime directory. It
accepts one newline-delimited JSON command per connection: `{"command":"status"}`
or `{"command":"shutdown"}`. Other declared commands, including API-key
generation, currently return `command_not_implemented`.

Shutdown cancels background work, drains tracked tasks and HTTP connections, and
removes the socket. The pending cleaner runs immediately and then every five
minutes, deleting pending objects/records at least one hour old in batches of
500. This retention window is separate from the two-minute upload URL lifetime;
confirmed files are not automatically cleaned up.

## Upload API

`POST /v1/uploads` accepts JSON with `size_bytes`, `mime_type`, and `filename`.
Sizes above 100 MiB and empty or overlong metadata return HTTP 400.
The response contains `request_id`, `uri`, `headers`, and `expires_at_utc`.
PUT the file to `uri` using the returned headers within two minutes.

`POST /v1/uploads/{id}/confirm` checks the uploaded size and MIME type, copies
it from `pending/` to `uploads/`, and removes the pending object. Repeated
confirmation is supported. It returns `public_uri` (a signed download URL valid
for 15 minutes), `expires_at` (URL expiry), and `new_storage_size_bytes` (total
confirmed bytes). Confirmed files have no automatic expiry.

`GET /v1/uploads?limit=50&after=<uuid>` returns `uploads` metadata in ascending
UUID order and `next_after` (null on the last page). The default limit is 50,
with a range of 1–100. Both pending and confirmed records are included.
Pagination uses `id > after`, ordered by `id`, with no SQL OFFSET. The cursor
need not refer to a record that still exists. This is a live listing, not a
snapshot; UUIDv7 ordering does not guarantee insertion order across machines.

`DELETE /v1/uploads/{id}` removes both object keys, then metadata, and returns
204; unknown IDs return 404. Storage failures retain metadata so deletion can
be retried. Bucket credentials need Head/Get, Put/Copy, and Delete permissions.
Browser uploads also require bucket CORS allowing PUT and the signed headers.
The service currently has no authentication, owner isolation or API CORS layer.
Keep it behind trusted access. Use the dashboard's same-origin API proxy for
browser requests; the bucket still needs its own CORS configuration.

Check compilation with `cargo check --locked` from this directory. Live startup
requires the configured database; actual uploads require the S3 service.
Install [Just](https://just.systems), then run `just check-rust` from the repository root
for formatting, lint, and regression checks (also run in CI).

## Planned sharing model

Files will be public or private. Public sharing will use stable URLs directly to
S3-compatible storage; no domain is chosen yet. Current `public_uri` values are
expiring signed download URLs, not permanent public links. See the
[architecture proposal](../../docs/file-visibility-architecture.md) before adding
visibility, authorization or delivery endpoints.
