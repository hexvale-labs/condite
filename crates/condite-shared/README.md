# condite-shared

MIT-licensed Serde wire types for the Condite HTTP API. No core, database, HTTP, UUID-generation, or time-zone dependencies.

Enable `typescript` only when generating declarations:

```sh
cargo run -p condite-shared --features typescript --example export-types
```

Generated JS-facing declarations use `bigint` for `u64`; raw HTTP JSON continues to use integer numbers. Use the SDK adapters for lossless JavaScript conversion.
