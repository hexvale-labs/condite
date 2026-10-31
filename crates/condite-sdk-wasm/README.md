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

Cross-origin deployments need API CORS permitting GET/POST/DELETE and Content-Type, plus bucket CORS permitting signed PUT headers such as Content-Type. Core's CORS policy is not changed by this SDK. Fetch sets signed Host and Content-Length automatically. Same-origin proxy deployments are also supported.

See [SDK tooling](../../scripts/sdk/README.md).
