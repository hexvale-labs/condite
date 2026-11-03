# API delivery, administration and configuration

Status: implemented and locally verified on 2026-11-03. Replaces the earlier
administrator-password proposal. Component READMEs and the client capability matrix describe current behavior.

## Confirmed requirements

- Public and owner downloads stream through core, validating each request.
  Keep originals private; stop publishing independent public bucket copies.
- Dashboard login accepts API keys. Ordinary keys see only their own files;
  root keys can view and manage everything. Vencord displays usage and limits.
- Multiple independently named root keys are configured in TOML, bypass logical
  quotas and grant administration. Storage/provider/protocol limits still apply.
- Dashboard includes key management, status, log levels, reload, deletion and
  shutdown; shutdown requires explicit confirmation. No Vencord administration UI.
- TOML is the configuration source throughout the project. Environment files
  supply values only through explicit interpolation so TOML can be committed.
- Keep a maintained client capability matrix and correct the Vencord navigation
  row's sizing and hover bounds to match adjacent Discord rows.

## Recommended architecture

HTTP/JSON on core's existing configurable TCP listener, with versioned root-only
administrative routes delegating to the same CommandBus as the Unix socket.
Docker dashboard calls core over service DNS; no shared socket volume or extra
raw TCP protocol. Keep Unix control for local recovery. Extend condite-ctl with
HTTP transport; SDK/control wire types stay independent of core dependencies.
A separate listener is unnecessary unless the operator needs network segregation.

Login exchanges a regular or root API key for an opaque, expiring session. The
dashboard stores only its session token in an HttpOnly cookie, Secure on HTTPS,
SameSite=Lax; no localStorage credential. Core stores token digests and checks
current key revocation/root membership on each authenticated request. Root
sessions identify both the stable key name and its credential generation, so
rotation/removal invalidates only affected sessions. Never put keys in URLs.
Protect SSR and proxy routes, enforce write-origin/CSRF checks, rate-limit login,
and return no-store responses. Sessions work across dashboard restarts/replicas.
Proposed lifetime: 12 hours absolute; logout revokes immediately.

Root ownership is stable across credential rotation, separate from ordinary key
ownership. Root-created files belong to the named root principal; root listing
includes owner identity and filters. Removing a root credential does not delete
files; remaining roots can manage them. Reject duplicate names and credentials.
Never log root secrets or return existing credentials during key inspection.
Generated ordinary keys are shown once. Administration adds paginated key listing
and usage inspection, which the current CTL does not provide.

## Configuration

Use toml-rs `toml` with Serde for Rust configuration. The current official docs
show 1.1.8+spec-1.1.0; pin a toolchain-compatible release during implementation.
`toml-spanner` is a modern alternative with accumulated errors and spans, but
Serde integration fits existing types and avoids a second data-model system.

Parse TOML first, then expand `${NAME}` in string values only, then deserialize
and validate. Missing references fail startup; expansion is single-pass, without
shell evaluation, implicit environment overrides or recursive substitutions.
`$${NAME}` denotes literal `${NAME}`. Reject unknown fields, invalid URLs, bind
addresses, duplicate roots, invalid durations, empty secrets and quota overflow.
Use strings with byte-size units for quotas. Report file/key locations without
including resolved secrets or secret-bearing source snippets. Do not derive a
secret-revealing Debug implementation. Relative paths resolve from the config file.

Include core, dashboard, ctl and both Vencord integrations in the configuration
inventory. Browser-facing settings cannot contain server secrets. Dashboard uses
a compatible Node TOML parser with equivalent interpolation/validation rules;
shared fixtures prevent divergent behavior. No SDK-library requirement for global
config: programmatic constructors remain supported. Native/browser client TOML
loading must respect available filesystem and environment capabilities.

Docker mounts TOML read-only and explicitly injects referenced secrets from .env;
Compose interpolation alone does not populate container environments. Document
that TOML containing literal secrets is still secret. Keep runtime/platform
variables distinct (PATH, HOME, container runtime settings).

Reload validates a candidate before applying supported settings. Invalid reload
keeps active configuration; report restart-only differences without secret values.
Root rotation/removal must have defined invalidation behavior. No writable web
configuration editor in this scope.

## Delivery and migration

Stream with bounded memory, range and HEAD support, safe MIME/disposition and
no-store headers. Do not hold SQL transactions for transfer duration. Owner
browser downloads use authenticated same-origin routes; SDKs receive streams or
download-to-file support. Preserve JS bigint precision. Keep direct signed PUT
uploads unless the user separately expands streaming to upload transport.

Inventory legacy public copy records and unfinished operations. Disable new copies
and remove old public objects using the existing durable cleanup machinery before
claiming revocation. Never rewrite applied migrations. Old external storage URLs
cannot be made API-mediated; the operator must decide cutover and old-link policy.
Public API base URL is explicit configuration, never inferred from untrusted Host.

## Alternatives

HTTP reuses clients, proxies and existing Axum infrastructure; raw TCP needs a new
remote protocol and shared Unix volumes complicate Docker permissions. Use HTTP.
API-key sessions meet the revised login requirement; the earlier administrator
password and an unauthenticated Docker bypass are excluded.

## Implementation decisions

- Keep Vencord settings UI with TOML import/export; native deployment settings use Electron userData/condite.toml.
- Sessions last 12 hours and persist in core SQL across restarts.
- Preserve owner download metadata, issuing API tickets instead of S3 URLs; retire legacy direct copies with durable cleanup.

## Sources

- https://docs.rs/toml/latest/toml/
- https://docs.rs/toml-spanner/latest/toml_spanner/
- https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet/

## Completion record

All three implementation tasks are complete on `feat/toml-auth-admin-api-streaming`.
Rust workspace tests and Clippy, WASM checks, SDK builds/runtime contracts and
dependency isolation passed. Dashboard checks/build, 36 unit tests and 24 browser
tests in each of production and development passed. Both Vencord integrations
passed their types, lint, builds and bridge tests. Docker Compose configuration,
release tooling and documentation links passed validation.

Execution decisions:

- Sessions persist for 12 hours; owner download metadata returns a 15-minute API
  ticket, validated on every request. Existing direct public links retire through
  durable cleanup; deployments must migrate configuration to TOML.
- Native Vencord deployment settings use `userData/condite.toml`; preference UIs
  offer TOML import/export while retaining Vencord's internal persistence.
- Dashboard loads the native addon with server-only `createRequire` to avoid SSR
  transformation of CommonJS. Deployment must retain the generated native package.
- Packaged Vencord bridge validation reused the freshly built native development
  SDK. The optimized native SDK profile was not separately benchmarked.

Final review found no critical issues. Its streaming timeout and stale credential
diagnostic findings were fixed and regression-tested; no findings were deferred.
Healthy downloads now use a header timeout followed by per-chunk idle timeouts,
with cancellation propagated upstream.

Live Discord rendering and a live S3 provider were unavailable; navigation sizing
and storage behavior were validated with local builds and fixtures. Newly added
labels use English fallback outside English/Spanish, as documented in the client
capability matrix.
