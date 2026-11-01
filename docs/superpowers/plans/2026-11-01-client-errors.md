# Client errors implementation plan

**Goal:** Readable errors with collapsed raw details in dashboard and Vencord.
**Architecture:** A shared MIT formatter understands structured SDK errors, native SDK JSON envelopes, and HTTP text. Each UI uses native details/summary disclosure. Preserve causes across native bridges.
**Approved design:** JSON title/detail; plain response bodies; friendly status fallback for empty bodies; full raw details, pretty-printed where possible.

- [x] Test formatter against JSON, native envelopes, text, empty HTTP responses, local errors and nested causes.
- [x] Implement formatter and localized fallback copy in the shared client catalog.
- [x] Apply disclosure components to dashboard, extension and standalone desktop plugin; retain caught errors and SSR errors.
- [x] Test formatter, native error propagation and browser disclosure; run applicable component checks.

Implementation notes: confirmation errors use a local modal because the upstream
string-only error slot cannot contain a disclosure. Opaque local causes stay in
raw details; structured HTTP causes also contribute to the readable message.

Validation: dashboard type check, build, unit tests and production/development
browser flows passed. Vencord builds, type/lint checks, 23 extension tests and
17 standalone tests with the real SDK passed. Live Discord rendering was not tested.
