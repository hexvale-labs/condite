# condite-sdk-wasm

MIT-licensed wasm-bindgen browser wrapper. Uses reqwest's browser backend without native TLS or Tokio. Build output is a local npm package in `dist/`.

```js
import init, {Client} from './dist/index.js';
await init();
const client = new Client('https://uploads.example.com');
const result = await client.upload({
  filename:'hello.txt', mime_type:'text/plain', size_bytes:5n,
}, new TextEncoder().encode('hello'));
```

Serve the generated WASM with `application/wasm`. API field names retain snake_case; methods use camelCase. Integer fields are `bigint`, including storage totals; avoid converting them to Number when precision matters. Errors expose `code`, `status`, `body`, and `request_id` where available.

Cross-origin deployments need API CORS permitting GET/POST/PATCH/DELETE and Content-Type/Authorization, plus bucket CORS permitting signed PUT headers such as Content-Type. Core's CORS policy is not changed by this SDK. Fetch sets signed Host and Content-Length automatically. Same-origin proxy deployments are also supported.

See [SDK tooling](../../scripts/sdk/README.md).

## Local privacy

```js
import { MetadataSanitizer } from './dist/index.js';
const sanitizer = new MetadataSanitizer({maxInputBytes:300n * 1024n * 1024n});
const eligibility = await sanitizer.assess(file); // File/Blob size + small header
if (eligibility.canAttempt) {
  const prepared = await sanitizer.prepare(file, file.name, file.type, {
    stripMetadata:true, anonymizeFilename:true,
  }, {signal:new AbortController().signal});
  await client.upload({filename:prepared.filename,mime_type:prepared.mimeType,
    size_bytes:prepared.sizeBytes}, prepared.data);
}
```

`canStripMetadata` returns boolean eligibility, not proof of removable PII or full
file validity. `strip` requires metadata cleaning; `decrypt(source, passphrase)`
returns authenticated plaintext bytes. `Client.uploadSanitized` combines the
pipeline with upload, stopping before presign on preparation failure. All privacy
byte fields/limits are bigint. Original source bytes remain intact.

Ship **all** generated JS/worker/WASM assets together, including
`privacy-worker.js`. Serve WASM as `application/wasm`; CSP must allow the local
module worker and WASM execution. Processing runs in a fresh worker with a
30-second timeout and optional AbortSignal; operations serialize per instance.
Default input/output limit is 300 MiB, not a hard browser memory guarantee. The
server's 100 MiB upload limit still applies to final bytes. Passphrases stay local;
there is no recovery service. See [coverage and encryption](../condite-metadata/README.md).

### Optional media

Install and host your chosen FFmpeg WASM assets explicitly; they are not bundled
or fetched by Condite. Supply a dedicated loaded `@ffmpeg/ffmpeg` instance to
`createFfmpegBackend`, then pass its result as the sanitizer's second argument.
The factory never chooses a CDN or downloads binaries. Audit your exact FFmpeg
build's licenses; its MIT JS wrapper does not determine the core's license.

Qualified setup: `@ffmpeg/ffmpeg` 0.12.15 with `@ffmpeg/core` 0.12.10. That core
has a legacy ffprobe exit-code/state-reset issue, requiring explicit compatibility
mode and fresh loads between operations:

```js
import {createFfmpegBackend, MetadataSanitizer} from './dist/index.js';
// ffmpeg is a dedicated FFmpeg instance; asset URLs are caller-owned/local.
const assets = {coreURL:'/ffmpeg/ffmpeg-core.js',wasmURL:'/ffmpeg/ffmpeg-core.wasm'};
await ffmpeg.load(assets);
const media = createFfmpegBackend(ffmpeg, {
  legacyProbeExitCode:true,
  reload:instance => instance.load(assets),
});
const sanitizer = new MetadataSanitizer({}, media);
```

Compatibility mode accepts only the known `-1` probe code with complete,
error-free stream/packet JSON and shared policy validation. Other nonzero results
fail. The adapter stages fresh VFS files between loads, cleans up afterward, and
terminates on abort/timeout. Color/rotation survive the qualified fixtures;
bitstream metadata is outside scope. Unsupported artwork/subtitle/data/side-data
variants reject. These are acceptance limits, not a hard FFmpeg worker heap bound.

## Sharing and retention

Upload requests accept optional `retention: {expires_at: string | null}`. Omit it
for 14 days; null means Never. Confirmation returns `{upload,
new_storage_size_bytes}` without sharing automatically. Use `download(id)` for an
owner-only temporary download and `createShare(id, {expires_at: null,
follows_upload: true})` for a direct share following file retention.
`listShares(id, {limit, after})`, `revokeShare(id, shareId)` and
`updateRetention(id, {expires_at})` manage shares and file deadlines. Returned
nullable deadlines are null, and sizes remain bigint. Privacy upload helpers
preserve the request's retention selection.
