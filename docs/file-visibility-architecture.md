# Proposed public and private files

Condite will support both public and private files. Most sharing is expected to
use public links pointing directly to S3-compatible storage. No public domain is
chosen yet. **This is a proposal; the current API has no visibility or auth.**

## Storage and delivery

Keep pending uploads and private files in a private bucket. Publish public files
into a separate public bucket (or an equally strict public-only prefix). A
separate bucket is the safer default: a public bucket policy cannot accidentally
expose private/pending keys. Private storage must block anonymous access.

Store an opaque object key and visibility in the database, not a signed URL.
Configure `PUBLIC_OBJECT_BASE_URL` server-side when a delivery endpoint is known;
combine it with the public key to return a stable, unsigned `public_url`. It can
be an S3 endpoint initially and a CDN/custom domain later. Keep this independent
from the S3 signing endpoint, since public and private delivery have different
access rules. Do not hard-code a domain in the dashboard.

Private downloads require authorization and a short-lived signed GET URL.
Presigned URLs are bearer capabilities: anyone holding one can use it until it
expires. Never call a signed URL a permanent public share link.

## Core changes before exposing visibility controls

1. Add owner/tenant authorization and a `visibility: public | private` field.
   Default new uploads to private. Require authorization for list, presign,
   confirm, delete and access changes; preserve the owner on confirmation.
2. Extend upload creation and metadata with visibility. Keep confirmation
   idempotent and separate it from download-link issuance. Add a download endpoint
   that returns a permanent public URL or an authorized expiring private URL.
3. Publish only after size/MIME validation. Give pending/private objects opaque
   keys; never infer authorization from unguessable UUIDs. Enforce storage policy
   independently of the UI. Do not put private URLs in shared caches/logs.
4. Make access changes explicit operations. Private → public copies to the public
   bucket, then commits visibility. Public → private removes public delivery
   before reporting completion. Use durable reconciliation for partial failures;
   do not silently change metadata while the old public object remains reachable.
5. Delete both possible copies and pending objects with retryable cleanup.
   Scope quota/byte accounting to logical records, not temporary physical copies.

Public → private cannot retract downloads or cached copies already shared.
Use short cache lifetimes if revocation matters; document the limitation.
Private signed links can remain usable until expiry even after an access change;
strong immediate revocation needs an authenticated download gateway instead.

## Browser and dashboard

The backend returns the delivery URL; the UI does not construct S3 URLs. Public
files get “Copy public link”; private files get “Create temporary link” with expiry.
Never prefetch private links or persist them in localStorage. Clipboard errors
must leave a selectable link as fallback. Direct uploads still use restricted
signed PUTs; configure bucket CORS for the dashboard origin and signed headers.

Until these API changes land, show the API's Pending/Confirmed states and label
all generated downloads as temporary links. Do not add pretend visibility toggles.
Plan migration of existing records explicitly; default to private unless existing
bucket policy proves that they are public. Test anonymous private denial, owner
isolation, public retrieval, expiry, failed access transitions and cleanup.
