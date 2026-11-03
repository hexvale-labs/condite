# Condite dashboard

Astro SSR with SolidJS v2 RC hydration, Tailwind and generated Node/WASM SDKs.
Use Bun 1.4.4+ for development and Node 22.23.4+ for production. Keep the experimental
renderer/compiler pins together; validate SSR and hydration on upgrades.

Run `just install` here, configure `dashboard.toml` from `dashboard.example.toml`,
start core separately, then `just run`. TOML contains `core_url`, `origins`, `host`
and `port`; `${NAME}` expands only explicit environment references. Missing
variables, unknown fields and invalid URLs fail. For another file, pass
`--condite-config /path/dashboard.toml`. Production: `node dist/server/entry.mjs`.
Keep the platform-specific native SDK beside the build.

Visitors log in at `/en/login` using their own ordinary or root API key. The dashboard
exchanges it for a core-backed 12-hour session and stores only an HttpOnly,
SameSite=Lax cookie (Secure over HTTPS). No shared server API key or browser
localStorage credential is used. Logout revokes the session. Core checks key
revocation/root rotation on each request; sessions survive restarts and replicas.
Use HTTPS through a reverse proxy; configure trusted origins in TOML.

Ordinary accounts see only their files, usage and limits. Roots see all owners,
can filter by owner ID, manage keys and use server status, logging, reload,
administrative deletion and shutdown. Shutdown requires typing SHUTDOWN. Generated
key secrets are shown only in their creation response. Root credentials remain
managed in core's TOML.

Uploads use direct signed PUTs to private storage, requiring appropriate bucket
CORS. Downloads and public shares stream through core, validating each request;
owner tickets expire after at most 15 minutes. Sharing and retention controls are
available in list and grid views. Sizes remain bigint; the proxy forwards raw JSON
bytes. It restricts routes, request bodies, origins and forwarded stream headers.

Checks: `bun run check`, `bun run test:unit`, `bun run build`, `bun run test`,
`bun run test:dev`. Browser fixtures exercise the built SDKs without a real bucket.
Live provider, Discord and mobile hardware verification remain separate.

See [Docker examples](../../docker/examples/README.md),
[delivery guarantees](../../docs/file-visibility-architecture.md),
[client capabilities](../../docs/client-capabilities.md) and
[translations](../i18n/README.md).
