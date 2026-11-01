# condite-sdk

MIT-licensed async client for Condite's v1 HTTP API. Depends on `condite-shared`, not `condite-core`; no AWS, database, or server dependencies.

```rust,no_run
use condite_sdk::{Client, ListUploadsRequest, PresignRequest};

# async fn example() -> Result<(), condite_sdk::Error> {
let client = Client::with_api_key("http://localhost:8080", "your-api-key")?;
let page = client.list_uploads(&ListUploadsRequest::default()).await?;
let result = client.upload(&PresignRequest {
    filename: "hello.txt".into(),
    mime_type: "text/plain".into(),
    size_bytes: 5,
}, b"hello".to_vec()).await?;
# Ok(())
# }
```

Native hosts supply a Tokio runtime. Default `rustls` enables HTTPS; disable default features for HTTP-only hosts or browser WASM. `Client::with_client` accepts a configured reqwest API client. Signed storage requests use a separate client without its default headers.

Methods: `health`, `presign`, `list_uploads`, `put_upload`, `confirm`, `delete`, and `upload`. Base URLs may include a deployment prefix. Pagination returns `next_after` for the next request's `after`. No automatic write retries; `Error::Upload` retains the request ID and failed stage for recovery. Upload buffers must match the declared size (the API enforces per-key limits).

## JavaScript and build checks

See [SDK tooling](../../scripts/sdk/README.md) for Electron/browser builds, linting, tests, and size measurements. Rust integers remain `u64`; both JS adapters expose them as `bigint`. UUIDs, timestamps, and runtime spans are strings matching the server's wire representation.

## Local file privacy

`privacy` reexports the independent MIT `condite-metadata` API. Use
`privacy::assess(size, header, &config)` before buffering, then `privacy::prepare`
for metadata cleaning, random filenames and optional standard age encryption.
`upload_sanitized(request, bytes, options, config)` performs that pipeline before
presigning and returns `(UploadResult, Report)`. Failed cleaning makes no upload
request. It uses the final filename, MIME and byte length; the original request
size must match the source bytes. Processing is synchronous CPU work: in an async
host, prepare on a blocking worker and call `upload` with the result.

For optional local FFmpeg/ffprobe, call `privacy::prepare_with_backend` with a
configured `privacy::media::FfmpegBackend`, then upload its result. No binaries are
downloaded. The 300 MiB configurable processing limit is separate from the
server's API-key upload and storage quotas, including encryption overhead. See
[coverage, limits and encryption](../condite-metadata/README.md).

`Client::with_api_key` marks the Bearer header sensitive and disables API redirects
on native hosts. Credentials are sent only to the API client, never to signed
storage requests. JS constructors accept an optional second `apiKey` argument.
