# File privacy implementation plan

> Execute focused test-first tasks in the existing worktree. Independent format processors can be delegated; integration remains sequential.

**Goal:** Client metadata removal, filename anonymization and optional passphrase encryption across native Rust, Electron and browsers.

**Architecture:** Independent MIT `condite-metadata` crate; SDK orchestration; Node and browser adapters; localized dashboard options.

**Spec:** [Metadata design](../specs/2026-10-31-metadata-stripping-design.md).

## Constraints and decisions

- Defaults: 300 MiB input; existing 100 MiB upload limit unchanged.
- All transformations occur before presign; required stripping never uploads original on failure.
- Add standard age passphrase encryption/decryption, with fresh randomness and bounded scrypt work. No server keys, secret persistence or URL passphrases.
- Anonymized names use random identifiers and a validated short extension; encrypted outputs use `.age` and application/octet-stream. Original filename is not embedded in encrypted bytes.
- User clarified Discord: added a desktop Vencord plugin; dashboard upload controls remain unchanged.

## Tasks

- [x] Shared contracts and format processors: tests for canary metadata, malformed files, format-specific preservation, unsupported/signed/encrypted variants and budgets; implement JPEG/PNG/WebP, Office and PDF in focused modules.
- [x] Privacy pipeline: tests for clean→rename→encrypt order, standard age roundtrip, tamper/wrong-passphrase failure, randomized outputs, size boundaries and required-mode failures; implement reusable Rust API and sanitized upload.
- [x] Adapters: tests using actual Node/Electron/WASM packages; expose async privacy operations, worker Blob facade and local size preflight, consistent bigint types; regenerate package declarations.
- [x] Optional media: explicit FFmpeg adapter installation, matching policies and fixtures; verify missing-backend failures and actual remux outputs where runtime available.
- [x] UI: Vencord composer submenu with accessible upload options, toggles, transient passphrase confirmation and local decrypt/save flow; test errors, ordering and no plaintext/passphrase network leakage.
- [x] Verify: focused Rust tests, SDK Rust/WASM checks, SDK runtime/consumer checks, dashboard type/unit/build/browser checks, independent review, docs and diff checks.

## Review focus

Unknown metadata does not receive a false cleaning guarantee. Compressed bombs require pre-allocation budgets. No secret appears in Debug, reports or errors. Cancellation cleans worker resources. Encrypted overhead is included in upload-size validation.

## Review resolutions

- Reject malformed fixed rendering chunks before retaining them; image regression passes.
- Separate original input and remux output limits; worker regression passes.
- Recheck cancellation after an awaited media plan; browser qualification covers it.
- Save decrypted overwrites through a new private file and atomic rename; permission regression passes.
- Compare actual media bytes for changed status; idempotent-remux regression passes.

Native parser budgets are cooperative, not hard RAM/time isolation. PDF supports
only a qualified classic-xref subset. Browser FFmpeg core 0.12.10 requires explicit
legacy-probe compatibility and caller-controlled fresh loads. Component READMEs
record these limits. Live Discord UI and real-device mobile checks require an
authenticated client/hardware and were not claimed.

Final SDK runtime regression also covers fixed privacy error codes across the
WASM worker boundary while hiding unexpected exception details. Release sizes
and qualification evidence are recorded in [validation](../../file-privacy-validation.md).
