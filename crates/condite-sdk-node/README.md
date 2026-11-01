# condite-sdk-node

MIT-licensed napi-rs wrapper for Node and Electron main/utility processes. The addon uses its own Tokio runtime. Build output is a local npm package in `dist/` for the host platform; publish/cross-platform prebuild distribution is not configured.

```js
const {Client} = require('./dist/index.cjs');
const client = new Client('http://localhost:8080');
const result = await client.upload({
  filename:'hello.txt', mime_type:'text/plain', size_bytes:5n,
}, Buffer.from('hello'));
```

Do not import native addons directly into a sandboxed Electron renderer; expose selected operations through your application's preload/IPC boundary. Package the `.node` file outside ASAR or configure unpacking.

See [SDK tooling](../../scripts/sdk/README.md) for build and lint commands. API field names retain snake_case; methods use camelCase. HTTP failures reject with `code`, `status`, `body`, and `request_id` details. Runtime validation errors also reject with code `input`.

## Local privacy

```js
const { MetadataSanitizer } = require('./dist/index.cjs');
const sanitizer = new MetadataSanitizer({ maxInputBytes: 300n * 1024n * 1024n });
if (await sanitizer.canStripMetadata(fileBytes)) {
  const file = await sanitizer.prepare(fileBytes, 'photo.png', 'image/png', {
    stripMetadata: true, anonymizeFilename: true,
    // Optional passphrase supplied locally; never put it in URLs/settings/logs.
  });
  await client.upload({filename:file.filename, mime_type:file.mimeType,
    size_bytes:file.sizeBytes}, file.data);
}
```

`assess` returns candidate format/reason; `canStripMetadata` is its boolean helper,
not a full validation or PII-detection guarantee. Both accept bytes or Blob/File.
`prepareFile(path, options)` checks size before bounded reads; `strip` is required
metadata cleaning; `decrypt(bytes, passphrase)` returns authenticated plaintext.
`Client.uploadSanitized(request, source, options, config, mediaPaths)` combines
preparation and upload. Preparation failure never falls back to uploading original
bytes. Native work runs on N-API's worker pool, serialized per sanitizer instance.
Native processing currently has no cancellation interface.

Optional media: pass `{ffmpegPath:'/example/ffmpeg',ffprobePath:'/example/ffprobe'}`
as the sanitizer's second argument. Executables must be installed explicitly.
Node and Electron use the same addon API; keep the native addon in the trusted
main/utility process. Browser code uses the WASM package instead.

Privacy methods use camelCase local fields and bigint byte limits/sizes; HTTP
request fields remain snake_case. Limits default to 300 MiB, while uploads remain
limited to 100 MiB after processing. Passphrase encryption produces standard
`.age` files; decrypt locally with this SDK or an age-compatible tool. See
[exact coverage and exclusions](../condite-metadata/README.md).
