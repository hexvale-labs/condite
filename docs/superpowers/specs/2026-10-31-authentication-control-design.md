# Authentication and control CLI

Status: implemented and verified.

Builder review: no unresolved design questions. User approved and requested continuation.

## Scope

Add database-backed API-key authentication to upload routes, enforce per-key
upload and storage limits, and provide a small Rust `condite-ctl` executable.
Health remains unauthenticated. Public/private sharing remains future work.

## Approach

Recommended: persist API keys and upload ownership in core's database, and
authenticate HTTP requests using Bearer tokens. Keep CLI/protocol types free of
core's AWS and database dependencies. Use Clap for help, subcommands and argument
validation. A custom checked size parser gives the requested binary-unit semantics.

A CLI linked to core would unnecessarily pull AWS/database dependencies into the
utility. HTTP administration would introduce a second administrative security
boundary; the existing Unix socket is sufficient.

## Keys and ownership

Generate secrets with 256 bits of operating-system randomness. Store a SHA-256
digest, an opaque key ID, creation time, revocation state and both limits. Return
the secret once; never log it or return it from inspection commands. Authenticate
against current database state, without a stale credential cache. Reject missing,
malformed, duplicate and invalid Authorization headers with HTTP 401.

Scope list, confirm, delete and byte totals to the authenticated key. Other keys'
upload IDs return 404. User subsequently requested a database reset and mandatory
ownership: migration 0004 wipes upload metadata and API keys once, then makes
owner_id non-nullable. S3 objects are outside this reset.

Deleting a key revokes it immediately for subsequent requests, retaining its
ownership record and files. It does not delete stored objects. Already issued S3
URLs remain usable until expiry; revocation cannot retract those capabilities.

## Quotas and recovery

Defaults: 100 MiB per upload and 1 GiB total. Limits must fit the database and
S3's signed content-length representation; zero is allowed to disable new bytes.
All arithmetic is checked. Reject quota violations before authorizing storage.

Count pending and confirmed logical upload records toward total quota. Reserving
pending bytes prevents concurrent presign requests from oversubscribing storage.
Use database transactions and a per-key database serialization mechanism for
reservation and limit changes; a process-local mutex alone is insufficient.
Confirmation checks current limits again before promotion, without double
counting its reservation. Concurrent confirmation, deletion, cleanup and limit
changes must preserve quota accounting and existing S3/SQL retry recovery.

Lowering limits preserves existing files and reservations. A pending file that
violates new limits cannot be promoted. Deletion remains available while over
quota. Successful deletion/expiration releases the reservation only after storage
cleanup succeeds. Failed operations retain enough metadata for safe retries.

## Control interface

Commands:

```sh
condite-ctl status
condite-ctl shutdown
condite-ctl apikey generate --max-per-upload-size=100mb --max-total-storage-size=1gb
condite-ctl apikey update <key-id> --max-total-storage-size=2GiB
condite-ctl apikey delete <key-id>
condite-ctl upload delete <upload-id>
```

Updates require at least one limit and preserve unspecified limits. Generate
prints the secret and key ID; support structured JSON output for scripts.
Declared but unsupported socket operations must produce explicit errors.

Try `$XDG_RUNTIME_DIR/condite/condite.sock` first; when absent, try
`/run/condite/condite.sock`. Do not conceal permission errors by falling back.
Allow `--socket` to select an explicit path. Use bounded newline-delimited JSON,
connection/read/write timeouts, structured server errors and nonzero exit codes.
Restrict the server directory/socket permissions to the service user.

Accept case-insensitive B, KB/KiB, MB/MiB, GB/GiB and TB/TiB with binary
multipliers. Require a suffix; reject negatives, overflow and malformed values.
Use whole-number quantities initially and explain accepted syntax in `--help`.

## Compatibility and validation

Generate a new Toasty migration; preserve historical migrations and license
boundaries. Put changed wire types in condite-shared first and regenerate SDKs
when HTTP contracts change. Ensure SDKs and dashboard can supply Bearer tokens;
never expose a server-side dashboard credential to browser bundles.

Extend router/fake-S3 integration tests for authentication, revocation, owner
isolation, quota boundaries, concurrent reservations, updated limits, retries and
cleanup. Exercise the CLI against real temporary Unix sockets, including discovery,
errors and timeouts; test size parsing and help. Run component checks from
`docs/development.md`, including `just check-rust` and affected SDK/dashboard
checks. Update operational documentation to describe implemented behavior.

## Research

- [Clap derive API](https://docs.rs/clap/latest/clap/_derive/)
- [OS randomness](https://docs.rs/getrandom/latest/getrandom/fn.fill.html)

The implementer must verify Toasty's current transaction and locking APIs before
choosing the concrete database serialization implementation.
The installed Toasty 0.11.0 exposes transactions and transaction builders; this
alone does not establish the required isolation across supported databases.
The Rust SDK accepts a configured HTTP client with default headers, while the
dashboard proxy currently needs explicit server-side credential configuration.
