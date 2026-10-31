# Condite dashboard

Astro SSR administration console with SolidJS **v2 RC** hydration, Tailwind and
strict TypeScript. Bun is used for development; production runs on Node.

## Run

First build the real SDK packages from the repository root (see
[SDK prerequisites](../../scripts/sdk/README.md)):

```sh
bun install --frozen-lockfile
bun run build:sdk
cd frontends/dashboard
cp .env.example .env
bun run dev
```

Use Bun 1.4.4+ and Node 22.23.4+ (a supported Node LTS is recommended).
`CONDITE_API_URL` is the private core origin, defaulting to `http://127.0.0.1:8080`.
Open `/en/` or `/es/`. Set `DASHBOARD_ORIGIN` to the deployed dashboard
origin **before building** so Astro validates its host; loopback development hosts
are allowed by default. This setting is independent of future public file URLs. The API proxy stays on the dashboard origin; signed file
PUTs go directly to storage. Bucket CORS must allow the dashboard origin, PUT and
the signed Content-Type header. The proxy never rewrites signed storage URLs.

```sh
bun run check
bun run test:unit
bun run build
bunx playwright install chromium
bun run test
bun run test:dev
HOST=0.0.0.0 PORT=4321 node dist/server/entry.mjs
```

Browser tests start a local HTTP fixture and the production Node server;
`test:dev` runs the same checks against Bun development. They use
the actual compiled native/WASM SDKs; no database or bucket is required. Ports
4321/4322 must be available. Keep the generated SDK packages and dashboard
`node_modules` available beside the production artifact; the native addon is an
external platform-specific dependency. Build it for the target OS/architecture.
Do not run the standalone production server with Bun.

## Behavior and constraints

- SSR falls back to localized error states after five seconds without a core
  response. The native SDK does not expose cancellation, so its underlying
  request may outlive that rendering deadline.
- Files are SSR-rendered, then hydrated. List/grid is stored in the URL to avoid
  a hydration jump. Filters apply to the current cursor page. Dates use UTC on
  both sides; sizes preserve bigint precision.
- Uploads expose preparation/PUT/confirmation states and preserve failed request
  IDs. Pending records can be confirmed or deleted. The SDK cannot report byte
  progress. Files are limited to 100 MiB by the API.
- Current download links expire. Future files will be public or private; public
  sharing will use direct S3-compatible URLs, with no domain assumed yet. See the
  [visibility proposal](../../docs/file-visibility-architecture.md).
- Core has no authentication. Put the dashboard and API behind trusted access;
  the proxy's origin checks do not provide user authentication.
- To add a locale, extend `src/i18n/index.ts` with a complete typed catalog and
  update Astro's `i18n.locales`. Labels, errors and formatting share that locale.
- The renderer/compiler stack is experimental. Keep exact pins and the Bun lock;
  run SSR and hydration checks on upgrades. Do not use the official v1 adapter.
- Mobile viewport tests cannot prove real-device keyboard, safe-area or touch
  behavior; confirm those on hardware before deployment.
