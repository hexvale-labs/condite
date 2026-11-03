# Private files and API delivery

Originals remain in the private S3-compatible bucket. Public shares and owner
downloads stream through core; requests never redirect to signed S3 GET URLs.
Direct signed PUT uploads remain unchanged.

A share is an independent, random API bearer URL. Core checks its current status,
expiry and parent file before opening storage. Revoking one share immediately
blocks subsequent requests without changing other links. A transfer already
started may finish; revocation cannot erase bytes already downloaded or cached.

Owner download metadata returns a random API ticket lasting at most 15 minutes,
capped by file expiry. Every ticket request checks current key revocation or root
credential generation as well as file availability. Authenticated clients may use
`GET /v1/uploads/{id}/content` directly. Public, ticket and authenticated paths
support HEAD and single byte ranges, stream with bounded buffering, and release
SQL transactions before transferring bytes. Responses use no-store, nosniff and
safe content headers; only recognized raster images render inline.

Use `public_api_url` in core's TOML for externally reachable links. Never infer it
from a request Host. A separate public bucket is unnecessary. Keep originals
private and configure CORS for signed PUTs only.

Cleanup runs at startup and every 60 seconds. Expired files and failed storage
deletions retain durable retry records. Existing deployments retain legacy share
bucket/key records: cleanup retires old direct copies, including uncertain late
copies after parent deletion. Old storage URLs cannot be redirected through core;
back up the database, deploy the migration, allow cleanup to succeed and replace
old links. Failed deletes leave legacy URLs reachable until storage cleanup
succeeds. Do not discard reconciliation tombstones or rewrite applied migrations.
