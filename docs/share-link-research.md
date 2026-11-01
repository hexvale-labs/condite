# Direct S3 share links: research

Research only; no sharing behavior has been implemented. Requirements: multiple
links per file, individual immediate revocation, chosen expiry or no expiry,
and private uploads by default.

## Findings

- **Separate object per link:** retain a private original and server-side copy
  each share to a distinct random key. Delete that copy to revoke the link;
  acknowledge revocation only after deletion succeeds. AWS S3 guarantees strong
  consistency for subsequent reads after deletion. This costs one additional
  stored copy per active share. Never reuse keys or expose version-specific URLs.
- **Timed direct links:** sign each separate private copy with the requested
  expiry. Deletion revokes it independently, but AWS SigV4 URLs expire within
  seven days (sooner with temporary credentials). This cannot provide arbitrary
  durations or indefinite links.
- **Indefinite direct links:** publish separate copies in a public-only bucket.
  Deletion revokes them, but exact expiry needs another mechanism. S3 lifecycle
  deletion is asynchronous and rounds times to midnight; a Condite cleanup task
  also cannot enforce expiry while the service is down.
- **Per-share bucket policies:** date conditions can restrict access, but dynamic
  policy edits are a poor general solution: bucket configuration is eventually
  consistent and policy size is limited to 20 KB. This is also less portable
  across S3-compatible providers.
- **Gateway:** a private original plus a database lookup on every request gives
  arbitrary expiry and immediate denial of new requests after revocation commits.
  Stream bytes through Condite (or a separately deployed authorization gateway).
  Redirecting to a signed origin URL leaves a revocation window at the origin.

No design can recall downloaded copies, in-flight responses or Discord's cached
images. Direct-origin revocation claims require provider consistency guarantees;
AWS guarantees must not be assumed for every S3-compatible provider. Public
copies should send `Cache-Control: no-store`, without promising third-party cache
invalidation. Keep originals and pending objects inaccessible anonymously.

## Recommendation awaiting selection

For all requirements together, use a database-backed streaming gateway. If direct
S3 delivery is essential, separate copies provide immediate manual revocation at
the origin, with explicit constraints on timed links. A hybrid can use direct
public copies for indefinite links and the gateway for expiring links.

## Latest requirements (2026-11-01)

- No CloudFront or AWS-only delivery infrastructure: support S3-compatible
  providers. Indefinite shares must be direct S3 URLs. Streaming is acceptable
  for longer-lived timed shares where necessary, subject to the Vencord rule.
- All Vencord upload links must be direct S3 URLs, usable by anyone holding the
  link. Vencord uploads default to 14-day retention and expose retention controls
  in the upload options drawer alongside encryption/privacy controls.
- Signed URL lifetime and upload retention are separate. An unsigned public copy
  can remain available longer than seven days; scheduled deletion enforces its
  retention, subject to service availability and retry delays. Whether that
  expiry guarantee is acceptable is still awaiting user confirmation.
- Upload retention is separate from link expiry: default to deletion after
  14 days (superseding 21), with controls to shorten/extend by intervals, choose a date or retain
  indefinitely. Dashboard and Vencord must both expose these controls; Vencord's
  upload drawer should include them alongside encryption/privacy options.
  The default will later be configurable through `config.jsonc`; do not introduce
  that configuration system as part of this change solely for this setting.
- Both clients and all SDKs must cover the applicable API surface. No delivery
  approach is final until the user's resource-usage question is resolved.
- Explicitly disclose AI-assisted authorship in AI-NOTICE.md, dashboard footer
  and Vencord settings/about. This is a transparency preference, not a claim
  that the EU mandates labels for AI-written software.

Streaming should relay bounded chunks with backpressure rather than collect
whole objects in RAM. Each uncached download adds one S3 GET, an authorization
lookup, an upstream connection and a downstream response. The host receives and
sends the file bytes; transfer charges depend on storage provider and placement.
Uploads can continue to use direct signed PUTs independently of download design.

## CDN delivery alternative

A private S3 origin behind an authenticated CDN avoids relaying file bytes through
the application API. CloudFront signed URLs validate expiry before serving cached
content, but signatures alone do not implement per-link immediate revocation.
A viewer-request authorization function can check a share ID before every cache
lookup, then map authorized shares to the same cached object. Strict revocation
requires an authoritative uncached authorization check and fail-closed behavior;
replicated edge revocation lists introduce propagation delay. CloudFront Functions
cannot make arbitrary network calls; use a suitable edge runtime (for example,
Lambda@Edge) when checking Condite on every request. Block direct origin access.

- [CloudFront signed URLs](https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/private-content-signed-urls.html)
- [Viewer-request authorization before cache lookup](https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/function-code-choose-purpose.html)
- [CloudFront private origins](https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/private-content-overview.html)

## Primary sources

- [Presigned URLs and expiration](https://docs.aws.amazon.com/AmazonS3/latest/userguide/using-presigned-url.html)
- [S3 consistency](https://aws.amazon.com/s3/consistency/)
- [CopyObject](https://docs.aws.amazon.com/AmazonS3/latest/API/API_CopyObject.html)
- [Lifecycle timing](https://docs.aws.amazon.com/AmazonS3/latest/userguide/troubleshoot-lifecycle.html)
- [Bucket configuration consistency](https://docs.aws.amazon.com/AmazonS3/latest/userguide/Welcome.html)
- [Bucket policy size](https://docs.aws.amazon.com/AmazonS3/latest/userguide/add-bucket-policy.html)
