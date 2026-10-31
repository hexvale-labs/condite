# Dashboard Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement task by task.

**Goal:** Ship the approved fast, accessible SSR admin console with list/grid files.
**Architecture:** Astro on Node renders generated native SDK data. Solid v2 hydrates
the UI and loads the WASM SDK for direct uploads and same-origin API interactions.
**Tech Stack:** Strict TypeScript, Astro, Solid v2 RC, Tailwind 4, Bun development,
Node production, Playwright integration checks.
**Spec:** `docs/superpowers/specs/2026-10-31-dashboard-design.md`

## Global constraints

- Exact Solid/compiler pins; no Solid 1 fallback or client-only rendering.
- Generated SDK contracts, bigint precision, all five existing API methods.
- Admin-style list/grid parity; i18n; keyboard/touch usability; stable hydration.
- Public/private files are planned backend work, not a fabricated UI capability.
- Prefer speed and utility; no gradients or decorative/long animations.

## Review focus

- Partial uploads remain recoverable after failed PUT or confirmation.
- Long filenames and translated labels do not overflow mobile layouts.
- URL state and dates remain identical across SSR and hydration.
- Proxy cannot forward arbitrary URLs or mutation requests from another origin.
- SDK size integers above the JS safe range survive transport and display.

## Tasks

### 1. SSR integration and SDK boundary

Files: `clients/dashboard/{package.json,astro.config.ts,tsconfig.json}`,
`src/lib/{server,client}.ts`, `src/pages/api/[...path].ts`, `tests/`.

- [x] Build the existing generated SDK packages; install exact framework versions.
- [x] Write integration tests asserting SSR file content, bigint and proxy boundaries;
      observe failure before adding the page/endpoint implementation.
- [x] Implement native server client, lazy WASM client and constrained proxy.
- [x] Verify typecheck, production build and a hydrated v2 island on Node.

### 2. Localized admin console

Files: `src/{components,layouts,pages,styles,i18n}/`, `tests/dashboard.spec.ts`.

- [x] Write failing browser checks for list/grid, locales, upload/confirm/delete,
      cursor paging, partial failure recovery and keyboard confirmation.
- [x] Implement typed English/Spanish catalogs, Astro shell and Solid islands.
- [x] Use Tailwind, Astro fonts, reserved feedback/icon space and URL view state.
- [x] Verify both views, both locales, JS-disabled SSR, a11y and layout shifts.

### 3. Documentation and deployment

Files: `clients/dashboard/{README.md,AGENTS.md,.env.example,.gitignore}`,
`docs/file-visibility-architecture.md`, root `README.md`.

- [x] Record Bun dev / Node production, SDK prerequisites and exact RC risks.
- [x] Propose public/private delivery with separate access policies and migration.
- [x] Document current signed links as temporary, with no domain assumed.

### 4. Final verification

- [x] Run complete dashboard tests, typecheck/build and existing SDK contract suite.
- [x] Smoke-test Bun development and Node production using generated native SDK.
- [x] Obtain a fresh code review and fix material findings with regression tests.
- [x] Report actual checks and any real-device limitations; leave changes local.

## Final verification results

- Astro check: 20 files, no errors/warnings/hints; production build passed.
- Transport: 4/4; browser integration: 10/10 on Node 22.23.4 and Bun 1.4.4.
- SDK contracts: Node, Electron and browser WASM passed.
- Fresh review addressed oversized streaming requests, SSR deadlines and the
  hydration identity assertion. Signed-link input keeps 16px sizing on mobile.
- SSR deadlines do not cancel native SDK work; the SDK exposes no cancellation.
- Real-device touch/keyboard/safe-area checks remain manual. Changes left local.
