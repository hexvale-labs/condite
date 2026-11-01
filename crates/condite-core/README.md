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
| `S3_BUCKET` | Private bucket for pending uploads and originals |
| `S3_ACCESS_KEY` | S3 access key |
| `S3_SECRET_KEY` | S3 secret key |

Optional sharing configuration: `S3_PUBLIC_BUCKET` must name a **different** bucket;
`PUBLIC_OBJECT_BASE_URL` is its externally reachable HTTP(S) base, without query,
fragment or credentials. Without both, private upload/download still works but
sharing returns a configuration error. Provision policies separately: originals
must deny anonymous access; the public bucket permits object GET but no anonymous
listing, writes or version reads. Both buckets use the configured S3-compatible
endpoint and credentials. No AWS-only service or domain is required.

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
or `{"command":"shutdown"}`. Logging commands are:

- `{"command":"set_log_level","level":"debug"}` changes the live tracing filter.
  Target filters such as `info,condite_core=debug` are also accepted.
- `{"command":"reload"}` restores `RUST_LOG` from the process environment
  (default `info` when absent). Invalid filters leave the active filter unchanged.
  It also compares the required and optional storage variables with the startup snapshot.
  Changed, missing or non-Unicode values require restart; the response names the
  variables without revealing values. Logging is still reloaded, but the response
  is `{"type":"error","code":"restart_required","message":"..."}` so callers
  can report the partial reload and exit nonzero.

Reload does not reread `.env`, replace storage clients, or change the socket path.
Exports in another shell do not modify the running process's environment.
`set_log_level` is temporary: the next successful reload replaces its filter.
Library embedders must call `condite_core::logging::init()` to enable log control.
Use [condite-ctl](../condite-ctl/README.md) for these commands, API-key generation,
limit updates, revocation and administrative upload deletion. The bounded socket
protocol restricts the directory and socket to the service user (0700/0600).

Shutdown cancels background work, drains tracked tasks and HTTP connections, and
removes the socket. Cleanup runs immediately and then every **60 seconds** with
indexed cursor batches of 100. It deletes abandoned pending uploads after one
hour, expired shares and expired confirmed files. One failed storage deletion
retains its record for retry without blocking unrelated cleanup. A separate
indexed queue checks confirmed files’ pending keys after three minutes, then daily.
A PUT started before its signed URL expires may finish later; deleted files retain
opaque private-key cleanup records, also retried daily after an initial check. Uncertain remote copies retain reconciliation tombstones
even after their parent is deleted; they are never silently forgotten.

## Upload, sharing and retention API

All upload routes require `Authorization: Bearer <api-key>`; keys are generated
with [condite-ctl](../condite-ctl/README.md). Records and mutations are scoped to
the owning key. Revoking a key does not revoke previously published share URLs.

- `POST /v1/uploads`: `size_bytes`, `mime_type`, `filename`, optional `retention`.
  Omit retention for **14 days**; `{ "expires_at": null }` means Never; otherwise
  supply an RFC 3339 timestamp. The response's `uri` and `headers` authorize a
  direct signed PUT for two minutes. Per-key upload/storage limits apply.
- `POST /v1/uploads/{id}/confirm`: validate size/MIME, copy pending to original,
  retry pending cleanup, and return `{upload, new_storage_size_bytes}`. It does
  not publish the file or issue a download link.
- `GET /v1/uploads?limit=50&after=<uuid>`: owner metadata, ascending UUID cursor;
  limit 1–100 and nullable `next_after`. Includes pending and deleting records.
- `POST /v1/uploads/{id}/download`: an owner-authorized signed GET, valid up to
  15 minutes and capped by remaining file retention. This is a temporary bearer URL.
- `PATCH /v1/uploads/{id}/retention`: `{expires_at: timestamp | null}` sets a new
  deadline or Never. Reject past dates and files already being deleted.
- `POST /v1/uploads/{id}/shares`: `{expires_at: timestamp | null,
  follows_upload: boolean}` creates an unsigned direct S3 share. When following
  the upload, send null and use its current retention. Otherwise choose an
  independent deadline or Never, always capped by the file's lifetime.
- `GET /v1/uploads/{id}/shares?limit=50&after=<uuid>`: paginated share URLs,
  nullable effective expiry, and Creating/Active/Revoking/Revoked/Expired status.
- `DELETE /v1/uploads/{id}/shares/{share_id}`: revoke that copy only; 204 means
  storage deletion completed. An ambiguous unfinished publication returns 202
  and remains in background reconciliation; its URL is never exposed. Retry safely after failure. At most 20 live shares
  per upload; revoked/expired history remains until its file is deleted.
- `DELETE /v1/uploads/{id}`: persist deletion intent, remove all public copies and
  both private keys, then metadata. Failed operations retain recovery state.

Public copies cost additional physical storage; quotas count each original once.
Copies use random keys and `Cache-Control: no-store`. Only supported safe image
MIME types render inline; other content downloads as an attachment.
Expiry depends on Condite being online and storage deletion succeeding; cached
or downloaded copies cannot be recalled. Extending retention follows only links
without an earlier explicit deadline. See [architecture](../../docs/file-visibility-architecture.md).

Migrations give existing files a private 14-day grace period from migration time.
Back up persistent data before upgrading. The default may later move to
`config.jsonc`; that configuration system is not implemented here.

Bucket credentials need Head/Get, Put/Copy and Delete on the appropriate buckets;
no ListBucket operation is required. Browser uploads need bucket CORS permitting
PUT and signed headers. Core does not add cross-origin API CORS; use the dashboard
same-origin proxy or provision an explicit deployment policy.

Run `just check rust` at the repository root for format, lint and workspace tests.
Fake-S3 tests verify application behavior, not a provider's real bucket policies
or consistency guarantees. Verify those before deployment.

Set `CORS_ALLOWED_ORIGINS=http://localhost:4321,https://dashboard.example` to
allow browser API access from multiple exact origins. Unset means no cross-origin
access. Restart after changes. This does not configure S3 bucket CORS.
