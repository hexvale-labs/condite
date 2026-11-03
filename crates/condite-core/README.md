# Condite core

Core owns HTTP, SQL, private S3 storage and local/remote administration.

## Configuration and startup

Copy `condite.example.toml` to `condite.toml` and `.env.example` to `.env` in this
directory, configure provider values and root credentials, then run `just run`.
Use `cargo run --bin condite-core -- --config /path/condite.toml` for another file.
TOML is authoritative: `.env` supplies explicitly referenced `${NAME}` strings,
never implicit overrides. `$${NAME}` is literal; missing variables and unknown
fields fail startup. Expansion happens after parsing and cannot inject TOML.
Diagnostics omit secrets. Relative database paths resolve beside the TOML file.

Configure `bind`, `public_api_url`, `log_filter`, `sql_conn_uri`, `s3_endpoint`,
`s3_region`, `s3_bucket`, `s3_access_key` and `s3_secret_key`. Optional
`cors_origins` contains comma-separated HTTP(S) origins. Multiple `[[root_keys]]`
tables each contain a unique stable `name` and an independent `key` of at least
32 printable ASCII characters. Store secrets in ignored `.env` files and reference
them from committed TOML; literal TOML credentials are still secrets.

Root keys have unrestricted logical access and no account quotas. Provider and
signed-64-bit storage constraints still apply. Ordinary keys remain isolated and
are generated/managed through the dashboard or CTL. Root-created files belong to
the named root identity, which survives credential rotation. Removing a root
preserves its files; another root can manage them. Reload applies root credentials
and the log filter, invalidating sessions/tickets for changed or removed roots.
Storage/listener/CORS/public-URL changes require restart. Invalid candidates retain
the active configuration. Reload reads TOML; process environment values are fixed
at launch, so change externally injected secrets through a restart.

Database startup applies embedded Toasty migrations. Back up persistent data before
upgrading. Generate schema changes with `just migrate <name>`; commit SQL,
snapshots and history together. S3 initialization needs no network probe or list
permission. Use one private bucket; old public bucket settings are cleanup history.

## Source layout

- `main.rs`: binary entry point and optional `.env` loading.
- `lib.rs`: application startup, listener, and currently registered routes.
- `state/app.rs`: active runtime configuration (`condite-config::CoreConfig::load`),
  shared application state and connection initialization.
- `state/config.rs`: compatibility re-export of the shared configuration type.
- `condite-config`: validated TOML parsing and explicit environment expansion.
- `db/`: database and S3 connection setup.
- `v1/`: API modules, including upload URL generation.
- `control/`: command bus and Unix control socket.
- `toasty/`: embedded SQL migrations, snapshots and history.
- `tests/uploads.rs`: router and Rust SDK integration tests with fake S3.

## Health and runtime

`GET /v1/health` returns `healthy`, `started_at` and `runtime`. It reports process
state, not a live database or bucket readiness probe. HTTP binds to
the TOML `bind` address (default `0.0.0.0:8080`).

The local control socket is `$XDG_RUNTIME_DIR/condite/condite.sock`, falling back
to `/run/condite/condite.sock`. Use a writable, private runtime directory. It
accepts one newline-delimited JSON command per connection: `{"command":"status"}`
or `{"command":"shutdown"}`. Logging commands are:

- `{"command":"set_log_level","level":"debug"}` changes the live tracing filter.
  Target filters such as `info,condite_core=debug` are also accepted.
- `{"command":"reload"}` validates TOML, applies roots/logging and names any
  restart-only differences without exposing values.

`POST /v1/control` accepts the same JSON commands over HTTP, requires a root key
or root session and delegates to the same command bus. It supports status, reload,
log filters, shutdown, key generate/list/inspect/update/revoke and deleting any
upload. Keep HTTP behind TLS; private Docker service traffic can use HTTP.
The Unix socket remains available for local recovery with 0700/0600 permissions.
See [CTL](../condite-ctl/README.md).

`GET /v1/account` returns identity, role, confirmed/reserved/charged usage, and
limits (`null` means unlimited). Root responses include separate global usage.
Root upload listing includes all owners, with an optional `owner_id` UUID filter.
`POST /v1/sessions` exchanges a key for an opaque 12-hour session;
`DELETE /v1/sessions/current` revokes it. Core stores token digests in SQL and
rechecks ordinary-key revocation/root membership on every request. The dashboard
keeps the token in an HttpOnly cookie. Sessions survive dashboard/core restarts.

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
- `POST /v1/uploads/{id}/download`: an owner-authorized API download ticket, valid up to
  15 minutes and capped by remaining file retention. This is a temporary bearer URL.
- `PATCH /v1/uploads/{id}/retention`: `{expires_at: timestamp | null}` sets a new
  deadline or Never. Reject past dates and files already being deleted.
- `POST /v1/uploads/{id}/shares`: `{expires_at: timestamp | null,
  follows_upload: boolean}` creates an API-streamed share. When following
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
PUT and signed headers. Configure `cors_origins` for cross-origin API clients, or use the dashboard
same-origin proxy or provision an explicit deployment policy.

Run `just check rust` at the repository root for format, lint and workspace tests.
Fake-S3 tests verify application behavior, not a provider's real bucket policies
or consistency guarantees. Verify those before deployment.

Set `CORS_ALLOWED_ORIGINS=http://localhost:4321,https://dashboard.example` to
allow browser API access from multiple exact origins. Unset means no cross-origin
access. Restart after changes. This does not configure S3 bucket CORS.
