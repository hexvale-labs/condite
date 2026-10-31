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
| `S3_BUCKET` | Bucket used for uploads and the startup connection check |
| `S3_ACCESS_KEY` | S3 access key |
| `S3_SECRET_KEY` | S3 secret key |

Startup opens the database connection and initializes the S3 client concurrently.
The S3 check lists at most one object, so credentials need permission to list the
configured bucket. Missing environment variables or connection failures stop
startup.

## Source layout

- `main.rs`: binary entry point and optional `.env` loading.
- `lib.rs`: application startup, listener, and currently registered routes.
- `state/config.rs`: runtime settings and required environment-variable loading.
- `state/app.rs`: shared application state and connection initialization. It also
  re-exports `AppRuntimeConfig` to preserve its existing import path.
- `state/env.rs`: environment-variable names and required-value lookup.
- `db/`: database and S3 connection setup.
- `v1/`: API modules under development. The presign handler is currently not
  registered with the router; the only registered route is `GET /`, which returns
  `Hello, world!`.

Check compilation with `cargo check --locked` from this directory. Live startup
also requires the configured database and S3 service to be available.
