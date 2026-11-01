# Private files and direct S3 shares

Original and pending objects stay in a private bucket. Uploads default to private
and expire after **14 days**. Retention can be changed to an exact UTC deadline
or Never. A future `config.jsonc` will configure the default; it is not implemented.

Each share has its own database record and cryptographically random public object
copy. Configure a separate `S3_PUBLIC_BUCKET` and `PUBLIC_OBJECT_BASE_URL` on the
same S3-compatible service. No chosen domain, AWS-only feature or CDN is required.
Never make the originals bucket public. Public policy should allow object reads
only, without listing, writes or version reads.

Confirmation validates the original without sharing it. Dashboard users choose
Share explicitly; Vencord creates a direct share for composer/privacy uploads.
Every client supports share creation/listing/revocation, owner downloads, file
retention and deletion. The API returns URLs; clients do not construct them.

## Lifetime and revocation

A share may follow its file's retention, use an earlier deadline, or have no
independent deadline. No share keeps its file alive. Expiring an individual share
removes its copy; expiring/deleting a file removes every copy and its original.
Logical quotas count originals once; every live share uses extra storage.

Condite runs indexed, bounded cleanup at startup and every **60 seconds**.
Storage failures and downtime delay expiration; this is an accepted limitation.
Revocation succeeds only after deleting the public copy. Storage consistency
varies by provider. Previously downloaded, in-flight and third-party cached data
cannot be recalled. Objects use `Cache-Control: no-store` to discourage caching.

Durable creation/deletion intents retain object keys across partial failures;
cleanup reconciles unfinished shares. Ambiguous remote copies keep their keys
scheduled for repeated deletion even after parent deletion, since a failed copy
request can still complete remotely. These unpublished URLs are not exposed;
revocation remains pending (HTTP 202) rather than claiming completion. Upload metadata stays until initial storage deletion succeeds. Opaque private-key
cleanup records survive logical deletion to remove late writes, without keeping
filenames. These checks run daily after the initial cleanup; failed deletes retry
on subsequent minute passes. Confirmed pending keys receive the same daily sweep. A file with copies awaiting deletion is not reported private.

Owner-only downloads use short-lived signed GETs, up to 15 minutes. These remain
bearer URLs until expiry and cannot be individually revoked like public copies.
Never prefetch or persist them in browser storage. Clipboard failures must leave
a selectable URL. Direct uploads use restricted signed PUTs and bucket CORS.

## Future option

A streaming download gateway could check expiry and revocation on every new
request while retaining only one private object. It adds host bandwidth and
availability requirements. It is intentionally deferred; current shared URLs go
directly to storage. [Research](share-link-research.md) records the alternatives.

See [core API/setup](../crates/condite-core/README.md) for routes and configuration.
Existing records receive a 14-day migration grace period. Test provider access
policies separately; local fake-S3 checks cannot prove anonymous original denial.
