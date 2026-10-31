# Core guidance

- Runtime configuration is loaded by `state/app.rs::AppRuntimeConfig::load_sync`.
  `state/config.rs` is not the active startup configuration; trace imports before
  changing settings. `lib.rs::app_router` is the HTTP route registry.
- Wire DTOs belong in `condite-shared`; handlers re-export them. Preserve SDK
  compatibility and keep AWS/Toasty dependencies out of shared types and SDKs.
- SQL and S3 writes are separate operations. Preserve retry recovery: confirmation
  retries pending-object deletion even for confirmed records; deletion removes
  both object keys before deleting metadata. Do not assume cross-store atomicity.
- Pending cleanup uses the record creation time, not the presigned URL expiry.
  Changes to lifecycle/key generation must account for the cleaner as well as
  presign, confirm and delete. Read the sharing proposal before changing delivery.
- Database startup applies embedded Toasty migrations. Generate schema changes
  with `just migrate <name>` at the root; commit migration, snapshot and history
  changes together. Do not rewrite previously applied migrations.
- Background work must observe the shared cancellation token and use the task
  tracker so shutdown can drain work and clean up the Unix socket.
- Prefer extending `tests/uploads.rs`: it exercises the router and Rust SDK with
  an in-memory database and fake S3. Run `just check-rust` from the root.
