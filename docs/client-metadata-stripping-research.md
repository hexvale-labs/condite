# Client-side metadata stripping research

Research date: 2026-10-31. Historical research preceding implementation. See the
[current coverage and API](../crates/condite-metadata/README.md) for shipped behavior.

## Recommendation

Build an optional MIT-licensed Rust metadata package shared by the Rust, Node and
browser SDKs. Start with JPEG/PNG/WebP and modern Office document properties;
qualify a PDF backend separately before promising PDF support. Keep audio/video
optional. Perform every transformation locally, before requesting an upload URL.

Expose a cheap eligibility check and a detailed assessment, with a configurable
300 MiB input ceiling. Treat eligibility as permission to attempt processing,
not proof that metadata exists or that processing will succeed.

This removes supported metadata, not all PII. Text, faces, voices, addresses in
screenshots, PDF form values, hidden worksheets, comments and revision contents
need separate policies. Never describe an output as “PII-free.”

## Existing Condite constraints

- [Rust SDK](../crates/condite-sdk/src/lib.rs) accepts a `Vec<u8>` and presigns
  before PUT. Both SDK presign validation and the server currently cap uploads
  at **100 MiB**. A 300 MiB sanitizer setting does not increase that limit.
- [Node adapter](../crates/condite-sdk-node/src/lib.rs) takes `Uint8Array` and
  calls `to_vec()`. The [browser adapter](../crates/condite-sdk-wasm/src/browser.rs)
  takes `Vec<u8>` across the JS/WASM boundary. Budget for buffer copies.
- SDK crates are MIT and independent of core's database/AWS dependencies. Keep
  the sanitizer independent too. Local assessment/report types do not require
  server wire-contract changes. If server-visible fields are later added, change
  `condite-shared` first.
- Sanitize first, derive the final byte length and MIME type, then presign, PUT
  and confirm. Changing bytes after presigning can invalidate signed lengths.

## Options

| Approach | Benefits | Costs and limitations | Verdict |
| --- | --- | --- | --- |
| Shared Rust processors with JS bindings | Consistent policy, native/browser reuse, permissive dependencies possible | We own format policy, corpus, fuzzing and compatibility | Recommended |
| JS-only processors | Fast browser experiment; pdf-lib and ZIP libraries available | Duplicate Rust path or inconsistent SDK coverage; still need sanitizer logic | Useful spike, weak long-term default |
| Existing desktop toolkit: ExifTool/mat2/qpdf | Broad format experience, useful independent verification | Processes/native dependencies; not a drop-in browser library | Test tools or optional native adapter |
| Commercial WASM PDF engine | Vendor-maintained PDF engine and support | Quote-based license; larger payload; Office/images still separate | Consider if PDF coverage becomes central |

### Images

Use container rewriting, preserving compressed pixel data where possible. Do not
make canvas decoding/re-encoding the default: it can change quality, color,
orientation, animation, HDR or output format and allocates decoded pixel buffers.

`img-parts` provides JPEG, PNG and RIFF/WebP container access, with EXIF/ICC helpers;
its license is MIT OR Apache-2.0. It is a building block, not a complete privacy
sanitizer: deleting EXIF alone misses XMP, IPTC and comments.
[API](https://docs.rs/img-parts/latest/img_parts/),
[license](https://raw.githubusercontent.com/paolobarbolini/img-parts/master/Cargo.toml).

- **JPEG:** remove EXIF GPS/device/author/timestamps, XMP (including extended
  packets), IPTC/Photoshop metadata, COM comments, thumbnails and unwanted trailer
  data. Define an explicit policy for APP segments; preserve rendering-critical
  JFIF/Adobe information. Reject unsupported variants rather than blindly deleting
  every APP segment.
- **PNG/APNG:** remove `eXIf`, `tEXt`, `zTXt`, `iTXt`, timestamps and unsupported
  private metadata. Preserve transparency, color and animation structures. Validate
  lengths/CRC and decide unknown-chunk handling. PNG metadata is not just EXIF.
  [PNG specification](https://www.w3.org/TR/png-3/).
- **WebP:** remove EXIF/XMP chunks, update RIFF lengths and VP8X feature flags,
  preserve image/alpha/animation chunks. Handle or reject unknown chunks explicitly.
  [WebP specification](https://developers.google.com/speed/webp/docs/riff_container).
- **Orientation:** removing EXIF may rotate how an image displays. Rebuild a minimal
  EXIF block containing only validated orientation, or normalize pixels with a
  separate transformation. Never retain the original EXIF blob just for rotation.
- **Color profiles:** retain a validated profile for fidelity, but report that
  arbitrary ICC profiles can contain identifying strings. A stricter mode would
  normalize to a known profile or reject it; simply dropping ICC can alter colors.
- **HEIC/HEIF/AVIF, TIFF/DNG/RAW, JPEG XL, animated GIF:** separate container/codec
  projects. Do not infer support from “image/*”. HEIC/AVIF metadata and orientation
  are structurally more complex; TIFF/RAW metadata can be coupled to image decoding.
  Return unsupported until individually qualified.

Baseline guarantee: targeted identity/location metadata removed for supported
containers, with rendering metadata preserved under a documented policy. This
cannot remove steganography or identifying visual content.

### PDF

A PDF needs a **full rewrite**, removing old revisions and excluded/unreachable
objects. ExifTool explicitly documents reversible incremental PDF changes; it is
unsuitable as the PDF sanitizer by itself.
[ExifTool PDF behavior](https://exiftool.org/TagNames/PDF.html).

Candidates:

- **lopdf (MIT):** best architecture fit for a shared Rust implementation. Supports
  PDF object manipulation and has WASM-related features. Evaluate with default
  threading/time features disabled as appropriate. WASM compilation, memory use,
  malformed-file behavior and privacy correctness remain unverified here.
  [Features](https://docs.rs/lopdf/latest/lopdf/),
  [license/build configuration](https://raw.githubusercontent.com/J-F-Liu/lopdf/master/Cargo.toml).
- **qpdf (Apache-2.0):** strong native rewrite alternative and independent test
  oracle. Browser use needs a qualified WASM build and maintenance; it is not a
  drop-in Rust/browser dependency. Its `--remove-info` preserves modification date,
  and `--remove-metadata` addresses catalog metadata; those flags alone are not a
  complete recursive sanitizer.
  [Options](https://qpdf.readthedocs.io/en/stable/qpdf-options.html),
  [license](https://raw.githubusercontent.com/qpdf/qpdf/main/LICENSE.txt).
- **pdf-lib:** JS-friendly low-level alternative. Its `updateMetadata` option
  needs deliberate handling. Clearing author/title via convenience setters is not
  sufficient evidence of XMP, old-object or embedded-file removal.
  [Load options](https://pdf-lib.js.org/docs/api/interfaces/loadoptions).
- **MuPDF.js:** official browser/Node WASM engine with documented scrubbing examples.
  Available under AGPL or commercial terms; obtain a commercial quote before
  adopting it into this permissively licensed SDK distribution. No public fixed
  price is established by the cited FAQ.
  [Licensing](https://mupdfjs.readthedocs.io/en/latest/faq/index.html),
  [scrubbing examples](https://mupdfjs.readthedocs.io/en/latest/deprecated.html).

Required PDF scope: remove trailer Info, XMP/Metadata streams throughout the
supported object graph, private identifiers and prior revisions; garbage-collect
excluded objects and write a fresh file. Preserve accessibility structure. Merely
unlinking metadata is insufficient if the writer serializes orphan objects.
Attachments, annotation authors, embedded images, form values, scripts and links
must be reported or separately handled; document-properties-only cleaning must
not imply those were cleaned. PDF/A metadata requirements may conflict with
removal; report conformance changes rather than promising preservation.

Initially reject encrypted and digitally signed PDFs. Even empty-password PDFs
can be auto-decrypted by libraries; detect original encryption before accepting.
Edits can invalidate signatures. A failed/unsupported PDF must never silently
upload its original bytes when cleaning was required.

### Microsoft Office and OpenDocument

For **DOCX/XLSX/PPTX**, use `zip` plus `quick-xml` in Rust and rebuild a fresh OPC
package. Both are MIT; `zip` documents WASM support with feature restrictions.
[ZIP documentation/license](https://docs.rs/zip/latest/zip/),
[XML documentation](https://docs.rs/quick-xml/latest/quick_xml/),
[XML license](https://raw.githubusercontent.com/tafia/quick-xml/master/Cargo.toml).

First policy: clear creator, last editor, creation/modification/print dates,
revision counters, description/keywords/title if selected, company/manager,
template paths and custom properties. Handle core/extended/custom property parts,
package thumbnails, ZIP comments, extra fields and timestamps. Resolve parts
through relationships/content types rather than assuming fixed paths. Update
relationships and content types when removing parts. Use neutral ZIP timestamps;
do not insert the local user's name, paths or processing timestamp.
Microsoft documents `LastModifiedBy` as a package property.
[Property reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.packaging.ipackageproperties.lastmodifiedby?view=openxml-3.0.1).

**Properties-only cleaning does not anonymize an Office document.** Word revision
attributes and comments, PowerPoint notes/comment authors, Excel threaded comments,
person records, external links, custom XML and embedded objects can retain identity.
Microsoft's own inspector treats these as multiple categories. A broader mode
needs format-specific traversal and decisions about content removal; accepting
tracked changes automatically would alter the document's meaning. Unknown embedded
objects should prevent a strict recursive-cleaning success.
[Microsoft inspector scope and limits](https://support.microsoft.com/en-gb/office/collab-files/remove-hidden-data-and-personal-information-by-inspecting-documents-presentations-or-workbooks?nochrome=true).

Reject macro-enabled DOCM/XLSM/PPTM initially, legacy binary DOC/XLS/PPT, XLSB,
encrypted/protected or signed packages, and unsupported Microsoft formats such as
Visio/OneNote/Outlook. Do not open Office or execute macros. ODT/ODS/ODP can follow
with a separate ZIP/XML policy, including metadata and embedded-object handling;
they are not automatically covered by the OOXML implementation.

### Audio, video and other containers

MP3 ID3, FLAC/Vorbis comments, WAV INFO/BEXT and MP4/MOV location/device/time tags
are plausible later targets. Prefer format-specific remuxing without re-encoding.
GPS may also live in timed telemetry/data streams, subtitles or embedded tracks.

FFmpeg stream copy avoids transcoding, but requires deliberate global, per-stream,
chapter and data-track policy. `-map_metadata -1` alone is not an exhaustive scrub.
[FFmpeg mapping documentation](https://www.ffmpeg.org/ffmpeg-all.html).

Browser FFmpeg is expensive for a default SDK: its example loads approximately
31 MB of core assets, uses a virtual filesystem, and the FAQ notes higher resource
use for multithreading and a current 2 GB input limit for that implementation
(not a universal WASM limit). Node support was dropped from that wrapper.
[Usage](https://ffmpegwasm.netlify.app/docs/getting-started/usage/),
[FAQ](https://ffmpegwasm.netlify.app/docs/faq/).
FFmpeg licensing depends on the actual build and linked components; the MIT JS
wrapper does not make the WASM core MIT.
[FFmpeg license](https://www.ffmpeg.org/legal.html).

`mat2` is useful prior art and a native verification tool: broad format coverage,
but Python/native dependencies and LGPL-3.0-or-later licensing make it unsuitable
as the shared browser SDK engine.
[Project/dependencies/license](https://github.com/jvoisin/mat2/blob/main/README.md).
Do not claim generic ZIP, SVG, EPUB, DICOM or arbitrary file support. Each needs
its own policy; some carry identity in normal content rather than optional tags.

## Proposed SDK surface

Illustrative TypeScript; this is not an existing API:

```ts
const sanitizer = new MetadataSanitizer({
  maxInputBytes: 300n * 1024n * 1024n,
  profile: 'document-properties',
});
const assessment = await sanitizer.assess(file); // Blob/File, bounded reads
if (!assessment.canAttempt) {
  // reason: too-large | unsupported-format | backend-unavailable |
  // encrypted | signed | resource-limit | malformed | unsupported-feature
  return;
}
const cleaned = await sanitizer.strip(file);
// cleaned: { data, mimeType, sizeBytes, changed, report }
// Presign using cleaned.sizeBytes, then upload cleaned.data.
```

Expose `canStripMetadata(file): Promise<boolean>` as a convenience wrapper over
assessment. It means “supported attempt within policy,” not “PII detected” or
“success guaranteed.” Keep `hasRemovableMetadata` a separate optional finding,
with `unknown` allowed. Already-clean supported files may return true and then
`changed: false`. Assessment is local and must not presign or upload.

- Reject size **greater than** 314,572,800 bytes before `arrayBuffer()`/WASM copy.
  Equal-to-limit files remain eligible subject to other limits. Keep JS size
  settings/results as bigint, consistent with current adapters.
- Browser assessment starts with Blob size and a bounded signature read. ZIP needs
  bounded tail/central-directory and selected-entry reads; PDF feature detection
  may require more parsing. If the inspection budget is exhausted, return a
  conservative reason, not a claim that the file is safe.
- Rust accepts size plus a seekable source; Node can offer a file-path/handle
  adapter. Byte-buffer APIs remain useful, but cannot undo memory already allocated
  by a caller. Repeat validation against the actual bytes at strip time.
- Use content signatures and parsed container structure; MIME/extension are hints.
  Do not classify every ZIP as Office. Detect unsupported subformats explicitly.
- Make profiles format-aware: `image-metadata`, `document-properties`, and later
  `extended` with a precisely listed scope. Report retained categories, exclusions,
  policy version and changed status, never actual removed names/GPS in logs.
- Add a high-level upload option such as `metadata: { mode: 'required', ... }`.
  Required mode blocks upload on failure. Best-effort mode, if offered, must return
  an explicit skipped/failed outcome; never quietly fall back to original bytes.
- Preserve the caller's original file; output a separate buffer/Blob. Filename
  cleaning is a separate explicit option: names can contain PII and currently go
  to Condite in presign requests.

## Memory, CPU and distribution cost

Whole-file buffering is **not inherently necessary** for every format. JPEG/PNG
chunk filtering can be streamed; ZIP can copy unchanged compressed entries while
rewriting selected parts. PDFs usually require substantially more structure in
memory. Given today's buffered upload APIs, start buffered but avoid making that
an architectural requirement for future source/sink adapters.

The 300 MiB ceiling is an input policy, not a memory bound. Two simultaneous full
buffers already consume 600 MiB; four consume 1,200 MiB before parser overhead.
Compressed Office/PDF content can expand far beyond input size. Even a tiny
compressed image can require huge memory if decoded.

Add independent limits for total inflated bytes, largest inflated entry/stream,
entry count, nesting depth, XML depth, PDF objects, output size and processing time.
Enforce actual bytes read, not only claimed ZIP lengths. Reject duplicate/ambiguous
ZIP paths and unsupported compression/encryption. Avoid filesystem extraction.
Use a Web Worker with transferable buffers and termination on timeout/cancellation;
on native hosts keep CPU work off the async I/O executor. Default to one concurrent
sanitization. A worker improves responsiveness, not available RAM.

Lazy-load browser format modules; Cargo feature gates alone do not provide runtime
lazy loading within one WASM binary. Keep PDF/video out of the basic SDK payload.
Measure compressed and uncompressed package deltas using existing SDK size tooling.
No browser timings or bundle deltas were benchmarked in this research.

## Engineering and monetary estimates

Planning estimates, not measured delivery commitments or vendor quotes. Assumes
one experienced engineer, existing SDK build infrastructure, focused scope and
review/testing included. Cash examples use **$1,000 per engineer-day**; substitute
our actual loaded rate. Rows are incremental; calendar time can differ.

| Deliverable | Engineer-days | Illustrative labor |
| --- | ---: | ---: |
| Assessment API, budgets, adapters, worker/upload integration | 4–7 | $4k–7k |
| JPEG/PNG/WebP policy and compatibility corpus | 5–9 | $5k–9k |
| DOCX/XLSX/PPTX document-properties policy | 5–10 | $5k–10k |
| PDF backend qualification spike | 3–5 | $3k–5k |
| Production PDF properties rewrite after successful spike | 7–15 | $7k–15k |
| Extended Office identities, comments/embeddings policies | 10–20+ | $10k–20k+ |
| Optional audio/video backend and packaging | 10–20+ | $10k–20k+ |

A first release covering images plus Office properties is roughly **14–26 days**
($14k–26k at the example rate). Including PDF spike and implementation makes that
**24–46 days**, contingent on backend qualification. Broad “all file metadata”
coverage is an ongoing maintenance program, not a bounded first release.

Recommended permissive libraries have no per-file API fee; stripping adds no
server processing service. Client CPU/RAM/battery, SDK asset delivery, maintenance
and testing are the costs. Asset bandwidth formula: additional downloaded bytes ×
uncached loads × our delivery price. Storage/normal upload charges continue.
MuPDF commercial pricing requires a quote including browser redistribution and
native SDK use. Do not budget it as $0. License notices and transitive dependencies
must be reviewed for the chosen pinned builds; this research is not license clearance.
Reserve roughly 1–3 engineer-days/month initially for regressions, dependency
updates and new fixtures; actual support burden will depend on format diversity.

## Qualification before shipping

1. Spike image container rewriting and a PDF full rewrite on native and browser
   targets. Record bundle delta, wall time, peak memory and cancellation behavior
   on a desktop and a low-memory phone. Test ordinary and near-budget files.
2. Use synthetic canary names, GPS and device IDs in multiple metadata locations;
   inspect output with independent tools (ExifTool, qpdf, unzipped XML). Raw byte
   search is supplementary: compressed/encoded data can hide strings.
3. Test old PDF revisions, orphan objects, XMP, attachments, orientation/color,
   APNG/WebP animation, Office relationships, custom properties, embedded images,
   modern comments, unusual namespaces and non-UTF-8 XML. Verify documents still
   open/render correctly. Preserve accessibility where promised.
4. Test exact size boundaries, spoofed types, truncated files, signed/encrypted
   inputs, decompression bombs, cancellation and memory ceilings. Fuzz processors.
5. Integration-test that required-mode failures make **zero network requests**,
   and successful uploads presign/upload only the cleaned bytes and final length.
6. Publish an exact per-format removal/retention matrix and backend versions.
   Client enforcement is bypassable by other upload clients, so the server cannot
   assert that every stored file has been sanitized without separate verification.

At the research stage, no dependencies were installed, files processed or
production code changed. The next decision is whether the first release should cover only images
or include Office document properties while PDF qualification proceeds separately.
