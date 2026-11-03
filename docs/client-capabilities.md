# Client capabilities

Current baseline, 2026-11-03. **Yes** means implemented, not necessarily verified
against a live deployment. **Planned** is not shipped. Update this file whenever
an API/client capability changes or a new client is added; verify source before
promoting a planned feature.

| Capability | Dashboard | Vencord desktop | Vencord routing userplugin | Rust/Node/WASM SDKs | CTL |
| --- | --- | --- | --- | --- | --- |
| Upload and confirm | Yes | Yes | Yes | Yes | — |
| List/delete own uploads | Yes | Yes | Yes | Yes | Delete any upload |
| Owner download URL | Yes | Yes | Yes | Yes | — |
| Create/list/revoke shares | Yes | Yes | Yes | Yes | — |
| Retention changes | Yes | Yes | Yes | Yes | — |
| Health/status | Yes | Yes | Yes | Yes | Yes |
| Local privacy preparation | — | Yes | — | Rust/Node; browser scope needs audit | — |
| Automatic Discord attachment routing | — | — | Yes | — | — |
| Per-user API-key dashboard login | Yes, HttpOnly sessions | Existing local key setting | Existing local key setting | Existing bearer key support | — |
| Usage, total quota, per-file limit | Yes | Yes | Yes | Yes | List/inspect |
| API-streamed owner/public downloads | Yes | Yes | Yes | Yes | — |
| Root key management | Yes | Intentionally no UI | Intentionally no UI | Yes, control/controlJson | Generate/list/inspect/update/revoke |
| Shutdown/reload/log-level changes | Yes, root only; shutdown confirmation | Intentionally no UI | Intentionally no UI | Yes, control/controlJson | Yes, Unix/HTTP |
| Remote HTTP administration | Yes, root only | Intentionally no UI | Intentionally no UI | Yes | Yes |
| TOML configuration | Yes, server | Native file + preference import/export | Native file + preference import/export | Programmatic constructors | Yes |

The dashboard exchanges each visitor's API key for a persistent 12-hour core session.
Ordinary keys are isolated; named TOML roots manage all owners. Root credential
rotation/removal invalidates affected sessions and owner download tickets.

Downloads stream through core, including the API URLs returned by the existing
owner-download metadata method. Rust additionally exposes a streaming response;
Node/WASM expose byte downloads and raw `controlJson` to preserve u64 values.
Public shares use the same private originals; old public copies retain cleanup
records until removal succeeds. Signed PUT uploads remain direct.

Both Vencord integrations require desktop native SDK access; compiling the routing
UI for web does not make its Electron bridge work in a browser. Their preference
UIs remain available with TOML import/export; native deployment paths and optional
fallback credentials come from Electron userData/condite.toml. Exports contain
client credentials. Native executable paths cannot be imported from renderer UI.

Local fixtures verify protocol behavior; provider permissions, live Discord row
geometry, clipboard/native dialogs and mobile hardware require deployment checks.

Sources: [dashboard](../clients/dashboard/README.md),
[Vencord desktop](../clients/vencord/README.md),
[routing userplugin](../clients/vencord-extension/README.md),
[Rust SDK](../crates/condite-sdk/src/lib.rs),
[control commands](../crates/condite-shared/src/control.rs).
See the [implementation roadmap](superpowers/plans/2026-11-03-api-administration-config.md)
for scope and verification requirements.
