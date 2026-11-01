# Agent Guidelines

- Keep documentation short and concise.
- Avoid refactoring unrelated code unless explicitly asked.
- Write clean, readable code and add comments only when they are needed.
- Keep tests focused; prefer integration tests over unit tests when practical.
- When working extensively with a library or tool, consult its latest documentation.

- Originals stay private; independent public share links point directly to
  S3-compatible storage. Condite manages expiry cleanup every minute. Owner-only
  signed download links expire. See `docs/file-visibility-architecture.md` before changing sharing.

## Project context

- Read `docs/development.md` for the repository map and validation commands.
  Component READMEs describe current behavior; `docs/superpowers/` contains
  design/implementation records, which may include proposals already superseded
  by code. Keep implemented behavior distinct from plans.
- `condite-core` owns HTTP routes, database records and S3 operations;
  `condite-shared` owns wire types. SDK crates must stay independent of core's
  AWS/database dependencies. Preserve each crate's license boundary.
- Root Cargo commands default to core; use `--workspace` for workspace checks.
  Root npm scripts cover SDK tooling; run dashboard Bun commands in its directory.
- Change wire contracts in `condite-shared` first and regenerate SDK packages.
  JavaScript adapters expose u64 as bigint; avoid lossy JSON/Number conversions.
- Keep every client and SDK aligned with the full applicable API surface,
  including sharing and upload retention. Omit capabilities only when a human
  explicitly requests it or implementation is impossible; document any exception.
- Never commit credentials, local databases, generated SDK `dist/` packages or
  build artifacts. Document environment names using example values only.
- Run checks appropriate to the changed component (see development guide).
  For documentation-only changes, check local links and `git diff --check`.

## Just command design

- Apply the same command design to every justfile: short verbs with spaced
  arguments, such as `just install backend` and `just check sdk` at the root.
  Never add duplicate aliases such as `install-backend`, `check-sdk`, `setup`,
  or bare component names. Extend the existing general command instead.
- Keep `just help` beginner-friendly and make it the default. Component
  justfiles use the same verbs with their local component implicit.
- Keep implementations in one place; update help, docs and CI callers when
  changing commands. Use `just check tools [component]` for setup diagnostics.
