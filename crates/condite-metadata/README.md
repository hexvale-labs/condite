# condite-metadata

MIT client-side file privacy processors shared by the Rust, Node/Electron and
browser SDKs. No HTTP, database or AWS dependency. No claim of removing all PII:
identifying text, faces, voices and embedded content can remain.

```rust
use condite_metadata::{Config, PrivacyOptions, prepare};
# fn example(bytes: &[u8]) -> condite_metadata::Result<()> {
let options = PrivacyOptions {
    strip_metadata: true,
    anonymize_filename: true,
    passphrase: Some("an example passphrase, replace locally".into()),
};
let file = prepare(bytes, "photo.png", "image/png", &options, &Config::default())?;
// Upload file.data with file.filename, file.mime_type and the final byte length.
# Ok(()) }
```

Processing order: clean metadata → replace filename → encrypt. Options default
off. Failed required cleaning returns an error; it never returns the original
as a cleaned result. `report.changed` compares bytes, including structural
normalization. `metadata_stripped` means the format's cleaning policy was applied,
not that PII was detected. Reports contain categories, not removed values.

## Coverage

| Format | Cleaning | Preserved / excluded |
| --- | --- | --- |
| JPEG | EXIF identity/GPS, XMP, IPTC, comments, thumbnails, trailers | Compressed pixels, reconstructed orientation, reported ICC/rendering structures |
| PNG/APNG | EXIF identity/GPS, text/XMP, time, trailers | Pixels, alpha, animation, orientation and reported color/rendering structures |
| WebP | EXIF identity/GPS, XMP | Pixels, alpha, animation, orientation and reported ICC |
| DOCX/XLSX/PPTX | Core/extended/custom properties, thumbnails, ZIP metadata | Document content, comments/revisions, notes, links and embedded objects |
| PDF | Info, Metadata/XMP, IDs, old revisions, orphan objects | Page content/accessibility; annotation authors, forms, attachments and embedded image metadata are outside scope |
| MP3/FLAC/WAV/MP4/MOV | Optional FFmpeg remux removes container/stream tags and chapters | Audio/video, rotation/color; codec bitstream metadata is outside scope |

Support is conservative. Unknown image structures reject; ICC profile contents
are retained and are **not sanitized**. PDF accepts classic xref tables with direct
stream lengths; object/xref streams, hybrid references and indirect stream lengths
reject. Signed/encrypted PDFs reject, including empty-password encryption. Office
rejects macros/ActiveX, protected/signed/encrypted packages, strict OOXML, ZIP64,
multipart ZIP and noncanonical/percent-encoded relationships. Legacy DOC/XLS/PPT,
XLSB, HEIC/AVIF/RAW/TIFF, generic ZIP and other formats are unsupported. Cleaning
may invalidate PDF/A conformance; no accessibility structure is deliberately removed.

Images use bounded container rewriting without decoding or re-encoding pixels.
Office uses `zip`/`quick-xml`; PDF uses a qualified `lopdf` subset. Format-specific
privacy rules belong here rather than assuming a library clears every metadata
location. Native tests include malformed/container variants and synthetic canaries.

## Eligibility and budgets

`assess(size, header, config)` / `can_strip_metadata` use a small header to identify
**candidate families** and check input size. ZIP is an Office candidate, not proof
of a valid Office package. Full validation occurs in `strip`/`prepare`, which may
still reject. Metadata presence is not inferred. Media candidates return
`backend-unavailable` without a configured backend; `assess_with_backend` accepts
an explicit availability flag.

Default input/output limits: 300 MiB each. Default ZIP inflation: 512 MiB total,
64 MiB per entry, 10,000 entries; XML/PDF nesting is capped. PDF uses conservative
preflight object/xref limits and bounded loading. Input >314,572,800 bytes rejects;
equality remains a candidate. Check source size **before** reading/copying it.
Buffers already allocated by the caller cannot be undone. Decompressed data,
multiple buffers and parser overhead can use much more RAM than file size.
These are structural/acceptance limits, not a hard process-wide RAM limit.

Rust preparation is synchronous CPU work: use a blocking worker in an async host,
then upload asynchronously. Native parser limits are cooperative, not hard
interruptible deadlines. Browser SDK processing uses a terminating worker with
30-second timeout/cancellation. FFmpeg uses a separate process/worker and a
30-second operation deadline. One sanitizer instance serializes JS processing;
callers control concurrency between instances.

## Encryption and media

Passphrase encryption uses standard binary **age v1** files through the Rust `age`
library: scrypt N=2^16/r=8/p=1 (~64 MiB), authenticated payloads and fresh randomness.
Decryption rejects higher scrypt work factors to bound malicious headers. Other
age files with higher work factors may need an external age tool. Wrong passwords
or tampering return no partial plaintext. Original filenames are not embedded;
encrypted outputs append `.age` and use `application/octet-stream`. Optional
filename anonymization uses a random 128-bit name plus an allowlisted extension.
Passphrases are redacted in Rust Debug and zeroized on drop; caller/JS/IPC copies
cannot guarantee erasure. There is no server recovery or key storage.

`media::FfmpegBackend::new(ffmpeg_path, ffprobe_path)` uses explicitly installed
local binaries. Call `prepare_with_backend` / `strip_with_backend`; nothing is
automatically downloaded. Native remux stages plaintext in a private temporary
directory, removed afterward; this does not securely erase physical media.
Probing rejects artwork/subtitles/data tracks and unsupported side data rather
than silently discarding content. Packet count/probe-size budgets can reject long
files. Output bytes are checked again before acceptance. Builds and enabled
components determine FFmpeg licensing; its binaries are not part of this MIT crate.

See [SDK usage](../condite-sdk/README.md), [browser media setup](../condite-sdk-wasm/README.md)
and [research](../../docs/client-metadata-stripping-research.md).
