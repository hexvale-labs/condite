# condite core

The Rust service starts with `cargo run` from this directory. The binary loads
`.env` when present, then initializes application state before binding to
`0.0.0.0:8080`.

Required environment variables:

| Variable | Purpose |
| --- | --- |
| `SQL_CONN_URL` | Database connection URL passed to SeaORM |
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
- `state/config.rs`: runtime settings and required environment-variable loading.
- `state/app.rs`: shared application state and connection initialization. It also
  re-exports `AppRuntimeConfig` to preserve its existing import path.
- `state/env.rs`: environment-variable names and required-value lookup.
- `db/`: database and S3 connection setup.
- `v1/`: API modules, including upload URL generation.

## Upload API

`POST /v1/uploads/presign` accepts JSON with `size_bytes`, `mime_type`, and
`filename`. The filename is accepted for compatibility but is not stored yet;
objects use a generated UUID under `pending/`. Sizes above 100 MiB return HTTP 400.
The response contains `uri`, `headers`, and `expires_at_utc`; clients must PUT the
file to the URI with the returned headers within two minutes. Signing failures
return HTTP 500 without exposing credentials or SDK error details.

The service currently has no authentication or upload metadata persistence.
`GET /` continues to return `Hello, world!`.

Check compilation with `cargo check --locked` from this directory. Live startup
requires the configured database; actual uploads require the S3 service.
Run `../scripts/check-rust.sh` for formatting, lint, and regression checks.
