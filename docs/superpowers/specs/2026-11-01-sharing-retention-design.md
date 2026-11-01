# Sharing, retention and client polish

Status: approved 2026-11-01 and implemented. Current behavior and recovery
refinements are documented in [the architecture](../../file-visibility-architecture.md).

## Agreed behavior

Uploads are private by default. Sharing creates independent bearer URLs directly
to S3-compatible storage; anyone holding a URL can download its object. One file
may have multiple shares with independent expiry and revocation. No CloudFront,
AWS-specific delivery service or download gateway is required.

Upload retention defaults to **14 days**, starting at upload creation. Every client
can shorten or extend retention, choose an absolute date, or choose Never. This
is distinct from a share's expiry. A future `config.jsonc` will configure the
default; introducing that configuration system is outside this change.

Condite, not S3 lifecycle rules, deletes expired uploads and shares. Run cleanup
immediately on startup, then once per minute. Expired public copies can remain
reachable during downtime or storage failures; the user explicitly accepts this.
Revocation is complete only after the public copy is deleted successfully.
Downloaded copies, in-flight downloads and third-party caches cannot be recalled.

Vencord automatically creates a direct S3 share after successful confirmation,
then offers/copies/inserts that URL according to each integration's existing
workflow. Default share lifetime follows the file's retention, including Never.
Dashboard uploads remain private until Share is used. No message is auto-sent.

## Approaches

1. **Selected: private original plus one public copy per share.** Server-side S3
   copy and delete provide independent direct URLs with arbitrary lifetimes and
   Never. This uses extra storage and requires a public-only delivery bucket.
2. **Signed private copies.** Individually deletable but signed URL lifetime is
   provider/credential-limited; it cannot meet the chosen 14-day Vencord default
   and indefinite-link requirements consistently.
3. **Streaming authorization gateway.** Avoids duplicate objects and enforces
   expiry on new requests, but carries download bandwidth and requires Condite
   availability. Keep it documented as a future option, not part of this work.

See [research](../../share-link-research.md) for source links and CDN alternatives.

## Storage, ownership and recovery

- Retain pending and confirmed originals in the existing private bucket. Add
  server-side public bucket and public base URL configuration, independent of the
  signing endpoint. Use the same S3-compatible client and credentials initially.
  Never fall back to making the originals bucket public. Missing sharing config
  leaves private uploads usable and produces an actionable sharing error.
- Generate a fresh cryptographically random opaque object key per share. Return
  unsigned URLs under the configured base; never encode owner IDs or reuse keys.
  Public bucket policy permits object reads only; no anonymous listing, writes
  or version reads. Document required provider policies and consistency limits.
- Copy only confirmed, size/MIME-validated originals. Set public copy metadata
  explicitly, including safe content disposition and `Cache-Control: no-store`.
  Active/scriptable formats download as attachments; supported image formats
  retain inline behavior. Preserve encryption and metadata-cleaning output.
- Persist share intent/key before copying. Track creating, active, deleting and
  terminal revoked/expired states, so crashes cannot leave untracked public
  copies. Reconciliation retries ambiguous copies/deletions. A share operation
  returns a usable URL only after publication succeeds.
- Keep owner authorization on all metadata, download, retention and sharing
  operations. Serialize lifecycle mutations with the existing owner-key locking
  convention. Recheck state/expiry under the lock before cleanup or publication;
  deletion must not race retention extension or create an orphaned share.
- File deletion removes every known public copy and both possible private keys
  before deleting metadata. Partial failures retain retryable records. Key
  revocation does not silently make already published objects private.
- Logical upload quotas continue counting each original once. Public copies cost
  extra physical storage; document this and bound active shares per upload with
  a documented server-side safety limit chosen during implementation.

## Persistence and API

Update `condite-shared` first and regenerate types; preserve bigint sizes and
crate license/dependency boundaries. Generate additive Toasty migrations,
snapshots and history using the existing migration command.

- Upload records gain indexed nullable expiry and deletion/lifecycle state as
  needed for durable cleanup. Existing uploads migrate private, expiring 14 days
  after migration, giving owners a grace period rather than deleting old files
  immediately. Pending uploads retain the existing one-hour abandonment cleanup.
- Share records contain upload/owner, random key, creation timestamp, nullable
  expiry and lifecycle status. Retain terminal share metadata until the parent is
  removed so Manage links can explain revoked/expired entries.
- Extend upload creation with retention selection: omitted uses 14 days; explicit
  Never is distinguishable from omitted. Reject invalid/past dates and overflow.
- Confirmation only confirms the upload and returns accounting/metadata; it must
  not publish or issue an implicitly shared URL. Update all consumers of the old
  `public_uri` contract together. Owner-only private downloads have a dedicated
  short-lived presigned GET operation and are clearly labeled temporary.
- Add create/list/revoke-share operations and upload retention updates. List
  shares with their URL, expiry and status, including pending deletion failures.
  Repeated revoke/delete attempts are safe. Validate ownership with non-leaking
  not-found behavior. Bound/paginate lists rather than returning unlimited rows.
- Reject new shares for expired/deleting files. Effective share expiry is the
  earlier of its requested deadline and the file's current retention. Extending
  file retention does not extend an explicitly timed share; a share choosing
  'Until file expires' follows the current file deadline. Never on a share does
  not keep the underlying file alive. Shortening retention queues due cleanup.
- SDKs expose every user-facing HTTP operation in Rust, Node and WASM. Update
  orchestration and independent consumer fixtures. Dashboard proxy allowlists
  retain origin checks, timeouts, body limits and raw JSON byte forwarding.

## Cleanup

Use indexed expiry/status queries and bounded batches, without scanning all
uploads or issuing storage LIST requests. Process due shares, due confirmed
uploads and abandoned pending uploads. Recheck each candidate under lifecycle
locking. Do not stop the entire pass on one failed object; retry on later passes
without repeatedly selecting the same failed row in a tight loop. Prevent
overlapping passes, honor cancellation between batches, and use the task tracker.
Log counts/errors without bearer URLs, credentials or private filenames.

## Client interaction and design

All clients cover the applicable HTTP API; this invariant is recorded in root
AGENTS.md. Cover both `clients/vencord-extension` (manager/routing) and
`clients/vencord/condite.desktop` (privacy options), plus all SDK packages.

- File list and grid have equivalent Share, Manage links, Expiry, Download and
  Delete actions. Show private/shared status, retention deadline and cleanup
  failures accurately; an overdue copy awaiting deletion is not labeled private.
- Share opens a modal with duration choices, an exact date, Until file expires,
  and Never. Manage links lists creation, expiry, status, Copy and Revoke for
  each entry. Offer selectable URLs when clipboard access fails.
- Expiry offers add/subtract intervals from the current deadline, a new absolute
  date or Never. Default presets: 1 day, 7 days, 14 days and 30 days; allow custom
  interval amounts. Never can be replaced with an explicit deadline. Confirm
  shortening to an already elapsed date as deletion, rather than silently losing
  a file. Use UTC on the wire and explicit timezone labels in date controls.
- Both Vencord integrations get working API-key settings passed through their
  native bridges. Snapshot endpoint/key for an operation. Never log keys; explain
  that Vencord settings store them locally. Preserve environment-key fallback in
  the privacy integration when its API URL matches the configured trusted URL.
- Put retention selection beside privacy/encryption upload controls and expose
  management after upload. Composer routing uses the selected default retention;
  manager uploads offer per-batch overrides. Both retain failed request IDs.
- Match the Home navigation row's sizing, icon alignment, theme colors, hover and
  focus behavior; investigate disappearance rather than just recoloring it.
- Improve dashboard and Vencord manager spacing, responsive toolbar, grouped view
  toggle, loading/empty/error states and action hierarchy. Avoid duplicate errors
  and large unused gaps shown in the screenshot. No gradients or decorative motion.
- Apply installed Emil design/animation skills and Solid v2/Astro guidance. Use
  short opacity/transform transitions for dialogs, feedback and file arrivals;
  keep actions immediately usable, support reduced motion, gate pointer hover,
  preserve SSR/hydration, keyboard focus and all existing locales/RTL.
- Both plugin author records use `955090007335530506n`; keep the existing display
  name unless a verified replacement is available.

## AI and community documentation

Add AI-NOTICE.md and modest linked 'Written with AI assistance' disclosures in
the dashboard footer and both Vencord about/settings surfaces. State authorship
transparently without claiming an EU legal requirement or labeling user uploads
as AI-generated.

Add concise contributing, code of conduct, security policy, issue forms and PR
template. Contact goes through GitHub Issues or private security advisories only;
no email, Discord or other contact addresses. Enable GitHub private vulnerability
reporting if repository permissions permit; otherwise identify that setup step
and do not direct sensitive reports into public issues. Respect existing MIT/GPL
component licenses; a root license overview must not relicense the workspace.
Finish with a one-to-two-sentence GitHub description in a code block.

## Validation and remaining implementation choices

Use router/SDK integration tests with fake S3 for owner isolation, multiple links,
private defaults, public publication/deletion, expiry/Never, cleanup cadence,
retention races, migration grace period and storage/DB partial-failure recovery.
Validate Rust/workspace, regenerated SDKs and independent consumer. Cover both
Vencord integrations with their fixtures and upstream type/lint/build checks.
Dashboard verification includes check/build/unit tests and production/development
browser tests for SSR, hydration, keyboard, dialogs, locales/RTL and responsive
layout. Never claim fake S3 proves a real provider policy or live Discord behavior.

Exact route/DTO names, retry scheduling details, batch sizes and share safety cap
are left to the implementer. Do not introduce `config.jsonc`, streaming, CDN
infrastructure, a new authentication system or unrelated refactors.
