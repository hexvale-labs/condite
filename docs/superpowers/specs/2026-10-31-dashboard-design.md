# Condite dashboard

## Intent and scope

Build a small administration console in `frontends/dashboard`, using Astro SSR,
SolidJS **2**, and the generated Condite SDK packages. Bun manages dependencies
and development; production runs the standalone Astro server on Node.

The user approved a quiet slate/white/blue direction, refined to an admin console
with both list and grid file views, and the Node + browser WASM SDK approach.
The user approved this design with Tailwind, strict TypeScript, minimal motion,
and future public/private file support documented separately.

## Experience

- A compact admin shell with Files and Service navigation, a placeholder Condite
  monogram, language switcher, and service status. No invented account/settings
  pages or unsupported backend features.
- Files is the default page. A toolbar provides upload, refresh, status filtering
  and list/grid switches. List is the default; a URL parameter preserves the view
  across reloads and language changes without a post-hydration layout jump.
- List uses a semantic table. Grid uses a semantic list of compact metadata cards
  with reserved icon space, not fetched file previews. Both expose filename, MIME
  type, size, creation date, pending/confirmed status, and identical actions.
- Cursor pagination uses the API's `next_after`; any client filtering is explicitly
  limited to the displayed page. No fabricated global counts or storage quotas.
- Upload accepts files up to the actual 100 MiB API limit. It shows per-file
  preparing/uploading/confirming/completed/failed states. A file picker is always
  available; drag and drop is optional polish. Do not claim byte progress the SDK
  cannot report. Preserve partial request IDs so pending uploads remain recoverable.
- Confirm pending uploads, generate a fresh 15-minute download link through
  idempotent confirmation for confirmed uploads, and delete after explicit
  confirmation. Confirmation failures retain the record; delete failures keep
  the row visible. Never silently retry a mutation.
- Service shows health, service start time and runtime, with a refresh action.
  Network errors and unhealthy responses are distinct from an empty file list.

## File visibility and sharing

Files will be public or private. Public links will eventually point directly to
S3-compatible storage; no domain is chosen yet. The current API exposes only
short-lived signed links and has no visibility/authorization fields. Do not
pretend those are permanent public links or expose a nonfunctional visibility
switch. See `docs/file-visibility-architecture.md` for the proposed evolution.

## Visual system

Admin navigation and operational file states distinguish this from a drive clone.
Keep content left aligned; use a restrained slate navigation rail, spacious main
workspace, compact toolbar, and consistent table rhythm. On phones, navigation
becomes a small top section; actions wrap and the table scrolls within its region.

Proposed tokens: canvas `#f4f7fb`, surface `#ffffff`, ink `#17283d`, muted
`#526477`, accent `#245dcc`, borders `#d9e2ec`. Validate contrast in the rendered
UI. Use Public Sans for UI text with Astro-managed local delivery and optimized
fallback metrics. Use tabular numerals for sizes/dates; no decorative monospace.
Use Tailwind CSS. Prioritize fast, snappy, minimal, functional, responsive UI;
no gradients, decorative motion, long animations or card-based metric filler. Success/error states have
text and icons as well as color. Dark mode is outside the initial scope.

## Architecture and integration

Selected: native Node SDK for server data and WASM SDK for browser interactions,
including direct signed storage PUTs. Alternative: server SDK for every operation,
which avoids browser WASM but adds an upload server hop and memory pressure.

- Use current stable Astro with `output: 'server'`, a compatible Node standalone
  adapter, and an exact pinned matching `solid-js`/`@solidjs/web` RC pair.
- The official Astro Solid integration still peers on Solid 1.x. Evaluate
  `astro-solid-next@0.1.0` (published source inspected) before adopting it. Its
  renderer uses awaitable `renderToStream`, per-request island IDs and matching
  `Loading` boundaries on server/client. Pin the compatible compiler too. If it
  fails runtime checks, maintain a minimal local renderer using verified v2 APIs.
  Never fall back to Solid 1 or `client:only`.
- Astro loads initial health/list data through `condite-sdk-node`; render Solid
  islands on the server, then hydrate with `client:load`. No server-global user
  state; never import the native addon into a browser module.
- Browser mutations lazily initialize `condite-sdk-wasm`. Read generated package
  types and `condite-shared/bindings.d.ts`; do not duplicate the Rust wire schema.
- Provide a same-origin, narrowly routed API transport to the configured Condite
  origin for the browser SDK. Restrict upstream paths/methods to the existing API;
  never accept arbitrary upstream URLs. Bucket PUTs remain direct and require
  bucket CORS. Do not rewrite signed URLs or signed headers.
- Preserve `bigint` sizes through SDK calls, SSR prop serialization, and formatting.
  Any JSON boundary must preserve integers exactly. Validate browser SDK/proxy
  behavior with integers above `Number.MAX_SAFE_INTEGER`.
- Private configuration supplies the core origin; the browser receives only the
  same-origin API base. No storage credentials belong in this project.
- Core currently has no authentication. Document that the deployment must sit
  behind trusted access; this task does not invent an authentication system.

## Internationalization, accessibility and layout stability

Proposed initial locales: English and Spanish, with typed message catalogs and
locale-aware dates, sizes and counts. All UI, errors, control labels, metadata and
empty states are translated. Astro locale-prefixed routes set `lang` on SSR and
language changes preserve the current page/view. Unsupported locales return 404.
Document how to add a catalog and route locale. Use logical CSS properties.

Use landmarks, a skip link, visible focus, accessible table headings, labeled
inputs, native buttons, and `aria-pressed` view controls. Delete confirmation uses
a native dialog with Escape/cancel and focus restoration. Announce async outcomes
in a persistent live region; restore focus after deleting a row. Keep actions
available without hover. Permit zoom and reduced motion; provide generous touch
targets and at least 16px input text.

Render the initial data and selected view before hydration, with the same locale
and timezone on both sides. Use Astro's font support with preload and optimized
fallbacks, bundled CSS, explicit logo/icon dimensions, and a reserved feedback
region. Retain previous data while refreshing. Avoid server-deferred above-fold
content or client-only viewport/date formatting. No remote thumbnails for now.

## Verification and deliverables

- Verify an actual production Node server renders filenames, health and localized
  controls without JavaScript; then assert hydration adds functioning controls
  without hydration warnings or replacing the initial file DOM.
- Integration fixtures exercise every API method via actual generated SDKs:
  health, pagination, presign, direct PUT, confirmation, fresh links, deletion,
  partial upload failures, retry/error handling and bigint precision.
- Browser checks cover list/grid parity and persistence, both locales, keyboard
  navigation/dialog focus, mobile-width overflow, accessibility scanning, and
  observed layout shifts during initial hydration/font load.
- Run typechecking and production build, test Bun development and Node production,
  and document any device-only checks that could not be run on hardware.
- Concise local README: build SDK first, Bun install/dev/check/build/test, Node
  start, configuration, proxy/bucket CORS and deployment boundary.
- `frontends/dashboard/AGENTS.md`: bleeding-edge Solid RC/compiler/Astro renderer
  risks, exact pins, v2 skill usage, generated SDK contracts, Bun development /
  Node production, and required SSR/hydration regression checks on upgrades.

## Builder decisions and limits

Settle component/file structure, icon selection, upload concurrency and test
fixtures during implementation. Keep changes scoped; adjust root scripts or
ignore rules only as needed. Do not add folders, renaming, share permissions,
retention editing, search across the whole bucket, quotas or analytics because
the current API does not support them. No deployment or PR is requested.

The current environment's Node 22.22.1/npm 9.2.0 does not meet Astro 7.4.4's
Node >=22.23.4/npm >=9.6.5 requirements. Use Bun for development as requested and
a supported Node runtime for production checks; do not lower Astro merely to
match an older preinstalled runtime. Verify native addon loading in Bun.
