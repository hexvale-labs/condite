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
