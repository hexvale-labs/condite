# Dashboard guidance

- Strict TypeScript, Astro SSR, SolidJS **v2 RC**, Tailwind CSS. Use `solidjs-v2`
  and `solidjs-v2-reviewer`; Solid 1 patterns are not interchangeable.
- This is a risky bleeding-edge integration. Keep Solid, `@solidjs/web`, compiler
  and renderer versions pinned together. The official Astro Solid adapter targets
  v1. Test production SSR **and hydration** before changing any of these versions;
  a successful build alone is insufficient. Never replace SSR with `client:only`.
- Use Bun for install/development; production is the Node standalone adapter.
  Keep native SDK imports server-only and WASM initialization lazy in the browser.
- Build and consume `condite-sdk`'s generated Node/WASM packages and shared types.
  Do not hand-copy wire types. Sizes are bigint; preserve full u64 precision.
- Files will be public or private. Most sharing will use stable public links
  directly to S3-compatible storage; no domain is chosen yet. Current API links
  are temporary signed URLs, not permanent public links. See
  `../../docs/file-visibility-architecture.md`; do not fake visibility controls.
- Favor fast, snappy, minimal, functional, responsive UI. Apply Emil Kowalski's
  design guidance. No gradients, decorative or long animations. Keep list/grid
  actions equivalent, visible keyboard focus, localized text and reserved sizing.
- Run `bun run check`, `bun run build`, `bun run test:unit` and `bun run test`.
  Verify JS-disabled SSR, hydration, API flows, both locales, keyboard and layout
  stability. Document real-device limitations honestly.

- For transport changes, read `src/lib/proxy.ts`: route allowlist, write-origin
  checks, body limit and raw-byte forwarding are intentional. Update focused
  transport tests alongside new routes; never JSON-round-trip u64 values.
- Keep `CONDITE_API_URL` server-side. `DASHBOARD_ORIGIN` is a build-time host
  validation setting, not a storage URL or authentication mechanism.
