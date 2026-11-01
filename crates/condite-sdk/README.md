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

`Client::with_api_key` marks the Bearer header sensitive and disables API redirects
on native hosts. Credentials are sent only to the API client, never to signed
storage requests. JS constructors accept an optional second `apiKey` argument.
