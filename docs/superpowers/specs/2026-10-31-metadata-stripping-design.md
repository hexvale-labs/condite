# Client metadata stripping

Status: approved; see [current coverage](../../../crates/condite-metadata/README.md)
for the qualified implementation. Builds on [research](../../client-metadata-stripping-research.md).

## Intent and scope

Strip supported identifying file metadata locally before SDK uploads in native
Rust, Electron main process through the Node addon, and browser WASM. No file
bytes or extracted metadata are sent to a cleaning service.

Implement JPEG, PNG/APNG, WebP, DOCX, XLSX, PPTX, PDF and optional MP3, FLAC, WAV,
MP4 and MOV cleaning. Other formats return unsupported. This removes defined
metadata categories, not identifying text, faces, voices or arbitrary hidden PII.
Existing upload methods remain available; cleaning is explicitly requested.

## Approaches

The research recommended shared Rust processors, and the user requested all
listed families across all existing SDK contexts. Use `img-parts`, `zip` plus
`quick-xml`, and `lopdf` behind a new independent MIT crate. Qualify PDF rewriting
before claiming it works. A JS-only implementation would duplicate Rust behavior;
command-line-only tools would exclude browsers.

For audio/video, use an optional FFmpeg adapter: a caller-configured local binary
for native Rust/Electron and a pinned, self-hostable ffmpeg.wasm build in browsers.
These have the same cleaning policy and fixtures, with platform-specific execution.
Media support requires these explicitly installed components in each context; it is
not available out of the box. Pin and test supported native versions and build
capabilities against a matching browser build. Do not introduce an automatic
binary download or require FFmpeg for other formats.
Review the exact FFmpeg build's license; keep its artifacts outside the MIT core.
If cross-platform fixtures fail for a container variant, reject that variant on
all platforms until corrected. No silent platform-dependent promise of support.

## Architecture and interfaces

- `condite-metadata`: independent parser/policy crate without HTTP, AWS or DB.
  Owns config, format identification, assessment, stripping and reports.
- `condite-sdk`: reexports local APIs; adds an explicit sanitized-upload method.
  Runs stripping before presign and uses the cleaned length and MIME type.
- Node addon: exposes corresponding methods; schedules CPU work away from the
  JS event loop. Electron uses the existing Node main-process integration.
- Browser package: JS Blob/File facade reads size before allocating bytes, then
  bounded header/tail data for assessment. Update `scripts/sdk/build.mjs` to ship
  separate facade and worker entrypoints, worker-safe WASM initialization, copied
  assets and platform-specific TypeScript declarations. Processing runs in a worker using
  transferable buffers; a byte-array entrypoint also exists for existing users.
- Optional media adapters: explicit backend configuration and availability checks.
  Browser assets are configurable same-origin URLs, not hard-coded public CDNs.

Proposed APIs (names are design choices): `MetadataSanitizer(config)`,
`assess(source)`, `canStripMetadata(source)`, `strip(source)`, and
`Client.uploadSanitized(request, source, config)`. Rust equivalents use snake case
and typed options/results. Source types are bytes or seekable readers in Rust,
bytes or a file handle/path in Node, and Blob/File or bytes in browsers.

`canStripMetadata` means the backend supports attempting the file within policy;
it does not mean removable metadata was found or that processing will succeed.
Assessment returns format, canAttempt, reason and optional hasRemovableMetadata
(with unknown allowed). Reasons distinguish oversized, unsupported, unavailable
backend, encrypted, signed, malformed and resource-limit cases.

Strip returns bytes/Blob, MIME type, u64/bigint size, changed and a policy-versioned
report listing removed/retained categories and limitations. Never include removed
names, coordinates or other sensitive values in reports/errors/logs. Already-clean
supported files succeed; `changed` means output bytes differ from input, including
structural normalization. A separate finding records whether targeted metadata was
found/removed. Original input is preserved.

## Cleaning contracts

- JPEG/PNG/APNG/WebP: remove EXIF identity/location, XMP, IPTC, text/comments,
  timestamps, thumbnails and unsupported metadata/trailer payloads using a defined
  container policy. Preserve compressed pixels, transparency and animation.
  Reconstruct only validated orientation if needed; preserve validated rendering
  structures. Retained ICC profiles are explicitly reported as a privacy limit.
  Unknown structures that cannot be safely classified cause rejection.
- Office: clean core, extended and custom properties including creator, editor,
  company/manager, dates, template paths and optional descriptive properties;
  remove package thumbnails and normalize ZIP timestamps/comments/extra fields.
  Rebuild the package with consistent relationships and content types. Report
  comments/revision content/notes/external links/embedded objects as outside the
  properties contract. Do not automatically accept revisions or delete user content.
  Reject macros, encrypted/protected/signed files and unsupported package variants.
- PDF: remove Info and supported Metadata/XMP throughout the object graph;
  remove optional persistent identifiers (regenerate only if required, without input
  identity), add no local timestamps or identifying producer values, remove obsolete revisions and unreachable
  metadata objects, and save a full rewrite. Preserve accessibility structure and
  page content. Detect signature structures throughout the supported object graph
  before rewriting. Reject signed/encrypted files, including empty-password encryption.
  Explicitly report annotations, attachments, forms and embedded image metadata
  outside the document-properties contract. Report potential PDF/A conformance loss.
- Audio/video: remux without re-encoding. Remove identifying container/stream tags,
  chapter metadata and telemetry/data tracks. Preserve audio/video and required
  playback structures, including rotation/color. Report retained subtitle/artwork
  content or reject unsupported track types rather than silently dropping content.
  Preserve subtitle/artwork content only with an explicit retained-content report.
  Codec bitstream metadata (such as SEI) is outside the container-metadata contract;
  do not claim to clean it. Reject identifying container/stream side data that the
  chosen remuxer cannot remove.

Each release documents the exact supported variants and removal/retention matrix.
No format is supported merely because a library accepts its extension.

## Resource and failure policy

Default max input: 314,572,800 bytes (300 MiB), configurable per sanitizer. Greater
than the limit returns false before full reads; equality remains eligible. Existing
100 MiB SDK/server upload ceiling remains separate and unchanged.

Input size alone cannot bound RAM. Implementation must also cap inflated bytes,
individual entries/streams, archive entries, XML depth, PDF object count, nesting,
output bytes and processing time. Proposed initial limits: 512 MiB total inflated
bytes, 64 MiB single inflated metadata part, 10,000 archive entries, depth 64,
100,000 PDF objects, 300 MiB output and 30 seconds per operation. These are initial
engineering choices to validate with fixtures; parser APIs must allow enforcement
before dangerous allocations, or the affected variant is unsupported. No recursive
embedded-file cleaning in this version. Default concurrency is one per sanitizer.

Qualification must prove parser limits apply before allocations, especially for
`lopdf`. A post-parse count is insufficient. Native in-process parsers have
cooperative deadlines, not hard interruption; do not claim a hard RAM/time bound
without an enforceable parser API or isolated process. Gate PDF support on this
qualification and document the actual guarantees.

Workers support cancellation and termination; native jobs check deadlines and
media subprocesses are killed on timeout. Rust APIs document any synchronous
processing and offer executor-friendly orchestration. Preflight reader operations
are bounded; inconclusive assessment returns a conservative resource-limit reason.
Revalidate actual bytes at strip time to handle changed files and spoofed metadata.

Sanitized upload is required-mode only initially: any skip, failure or unsupported
variant aborts before presign. No fallback upload of original bytes. On success,
final MIME/size drive presigning and the returned upload report contains the local
sanitization report. Filenames are preserved and documented as potentially
identifying; optional filename replacement is a separate caller decision.

## Validation and completion criteria

Use real synthetic canary fixtures across native Rust, Node, Electron and browser
Chromium with the actual generated SDKs. Exercise all supported families, verify
removed metadata with independent readers, and open/decode outputs for fidelity.
PDF tests include historical revisions and orphan objects; Office tests include
relationships and retained comment identities; image tests cover orientation,
color and animation. Media tests inspect global/stream/side-data and telemetry.

Test exact size limits without allocating enormous buffers, invalid configuration,
spoofed types, malformed files, encrypted/signed inputs, inflated-resource attacks,
missing media backends, cancellation and required-mode zero-network failures.
Integration tests confirm final cleaned bytes and lengths reach storage fixtures.

Run repository Rust/WASM/SDK checks, independent-consumer type checks, Node,
Electron and browser runtime suites. Record native/WASM asset-size deltas and
representative latency/peak-memory measurements. Only mark a family complete after
its processor and all three platform integrations pass. Unsupported variants and
unavailable optional media installations must be reported honestly in docs.

## Delivery order

1. Shared contracts, limits, eligibility, fixtures and SDK binding surfaces.
2. Image processors and cross-platform integration.
3. Office properties and PDF qualification/full rewrite.
4. Optional native/browser media adapters and identical policy fixtures.
5. Sanitized upload orchestration, complete SDK checks, size measurements and docs.

No server schema, sharing behavior or upload-limit changes are part of this work.
No commercial dependency purchase is authorized by this design. If a backend
cannot meet the safety or portability contract, report the concrete blocker before
substituting a paid engine or weakening the cleaning promise.

## Approved extension: upload options and encryption

User approved the metadata design and requested an extensible upload-options
submenu, filename anonymization and optional secret-passphrase encryption after
metadata removal. Repository inspection found dashboard uploads, but no chat UI;
placement was clarified as the Discord message box; the desktop Vencord plugin
now supplies that integration. Options start disabled and are captured per
batch. No secret or temporary-link persistence.

Use standard age passphrase encryption (Rust age 0.11), scrypt N=2^16/r=8/p=1
(~64 MiB KDF memory), authenticated streaming payloads and fresh randomness. Bound
decryption to the same work factor to avoid malicious KDF headers. Encrypt only
processed bytes; original filenames are never embedded. Encrypted uploads are
`.age` and application/octet-stream; filename anonymization is independent and
uses a random 128-bit identifier plus an allowlisted extension. Provide local
SDK decryption and a decrypt/save UI so downloads remain usable. Authentication
failure returns no partial plaintext. Passphrases are transient, masked, confirmed
before encryption, never sent in API/storage requests, links, reports or logs.
Rust secret values clear on drop; JS strings cannot guarantee memory erasure.
Recipients require the passphrase separately; no server recovery is provided.
