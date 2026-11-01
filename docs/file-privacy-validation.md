# File privacy validation

Validated 2026-10-31 on Linux ARM64. See [coverage](../crates/condite-metadata/README.md)
and [Vencord installation](../clients/vencord/README.md).

- Native/shared processors: synthetic canaries, historical PDF revisions, malformed
  containers, signed/encrypted inputs, compressed-resource budgets, random names,
  age roundtrips/tampering and required-mode rejection before network.
- Generated SDKs: actual Node, Electron 42 and Chromium WASM contracts; Blob size
  preflight, cleaning then encryption, bigint values and unchanged input buffers.
- Media: native FFmpeg n8.1.3 and browser FFmpeg 5.1.4 (`@ffmpeg/ffmpeg` 0.12.15,
  core 0.12.10). Five audio containers plus rotated/color MP4/MOV video fixtures;
  independent native decoding matches output, canaries disappear, abort and VFS
  cleanup work. Browser compatibility explicitly enables legacy probe handling
  with fresh caller-controlled engine loads. Reproduce via the
  [SDK tooling guide](../scripts/sdk/README.md).
- Vencord 8d64c8be (v1.15.15): full upstream type/lint/desktop build, workflow/native
  tests and actual SDK loopback upload/decrypt. Save-overwrite tests cover private
  permissions and protection of the encrypted original.
- Dashboard regression: type/unit/build checks and production/development browser
  suites cover SSR without JS, hydration, locales, keyboard and upload flows.
- Independent review findings were reproduced and fixed: malformed retained
  rendering chunks, remux input/output budget confusion, missed cancellation,
  decrypted overwrite permissions and byte-identical changed reporting.

Small-file Node release observation: 102-byte synthetic PNG, three sequential
iterations, median metadata processing 0.30 ms; metadata + anonymization + age
passphrase encryption 1,793 ms. Whole-process peak RSS was 146.68 MiB. This was a
single Linux ARM64 run during background compilation, not a large-file benchmark
or an isolated per-operation memory measurement. Scrypt's configured memory cost
is about 64 MiB regardless of file size. Real low-memory phones were not tested.

[SDK release sizes](sdk-sizes.json) record native/WASM assets for this toolchain;
external FFmpeg assets are additional. No hard process-wide RAM/time guarantee is
claimed for native parsers. No authenticated Discord UI, live bucket CORS or
real-device mobile smoke test was performed; build and loopback tests cannot prove
those environments. There were no live Discord messages or external uploads.
