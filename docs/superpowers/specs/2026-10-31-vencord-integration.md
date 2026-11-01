# Vencord integration: initial test implementation

Requested: evolve the verified blocking hook into an Electron SDK client, three
routing modes, per-file failure recovery and a Discord-native Uploads pane. The
user chose a full pane over a modal file manager. “Plicko” is treated as the
legacy name; destinations are Condite and Discord. Live-client testing remains.

## Research

Upstream references use the pinned
[`8d64c8b` tree](https://github.com/Vendicated/Vencord/tree/8d64c8be7e68616f8f116a6fff9480051aa2a0c5).

- `anonymiseFileNames/index.tsx`: `async uploadFiles(\i)` hook. Preserve the
  original method/receiver for Discord via a one-use batch bypass.
- `voiceMessages/index.tsx`: `CloudUpload` emits `complete` and `error`. Observe
  errors plus batch rejections, remove listeners on terminal events/stop, and
  recheck completion after recovery selection. No silent provider switching.
- `openInApp`, `directFileDownload.desktop`, `main/ipcPlugins.ts` and
  `VencordNative.ts`: native exports become IPC helpers through `PluginNative`.
  Load the SDK in main, with decimal strings across the bridge for u64 values.
- `webpack/common/modals.ts`: modern Modal/ConfirmModal, actions and close
  callbacks. Legacy `utils/modal` exports are deprecated. Use Vencord controls
  and Discord typography/theme tokens; no added decorative motion.
- `pinDms/index.tsx`: `.FRIENDS},"friends"` identifies the Home sidebar module.
  No general Home-page registration API was found in this pin. Add a sibling
  entry and independently mount a React portal in the main landmark; toolbox
  access then survives a sidebar mismatch. Avoid hashed DOM class selectors.
- `crashHandler`: lazy `saveDraft`/`clearDraft` actions. Save the latest draft
  using the originating channel ID after asynchronous work. The usual
  `insertTextIntoChatInputBox` targets the last composer without a channel ID.
  Active-editor synchronization still needs a live test.
- [Plicko](https://github.com/arcultus/plicko): reviewed drop/picker interception,
  provider modal and native bridge. Its aggregate size decision and filename-keyed
  result map do not suit per-file routing/duplicate names. Use ordered entries
  and the SDK; no HTTP implementation copied.
- [FriendsInVoice](https://github.com/mPhpMaster/FriendsInVoice): a custom page
  implementation using a server-list entry/fixed overlay, not a reusable Home
  sidebar page API. No code copied from that plugin.

## Limits

Checked 2026-10-31 against Discord's
[attachments FAQ](https://support.discord.com/hc/en-us/articles/25444343291031-File-Attachments-FAQ)
and [Nitro comparison](https://support.discord.com/hc/en-us/articles/115000435108-What-are-Nitro-Nitro-Basic):
Free 20 MB, Basic/Classic 50 MB, Nitro up to 1 GB. Older/localized pages still show
older values, and Discord explicitly experiments with allowances. Membership
values 0/1/2/3 map to None/Classic/Nitro/Basic in its
[user documentation](https://docs.discord.com/developers/resources/user#user-object-premium-types).

Fallbacks live in `src/limits.ts`. Prefer a named live limit helper if exported;
allow a settings override in MiB. Fallback server tiers 2/3 provide 50/100 MiB;
unknown experiments need the helper/override. Decide per file, including equality.
Condite's independent SDK/core maximum is currently 100 MiB.

## Implementation choices

- Package the native SDK in sibling `dist/condite-sdk`, outside the renderer;
  preserve its MIT license and keep core/AGPL dependencies out of SDK code.
- Expose every SDK operation. Use presign/PUT/confirm for staged UI progress;
  preserve error request IDs and pending records. Snapshot transfer endpoints.
  Reserve an unused API-key setting until SDK/API authentication exists.
- Append escaped, ordered Markdown links; keep duplicate files distinct. Remove
  successful attachments after draft insertion. Retain failed attachments and
  successful links after partial failure.
- Use a React portal in Discord's main area, restoring underlying visibility on
  close/unmount. No iframe. Match dashboard capabilities: listing, filtering,
  pagination, upload, confirm, temporary links/copy fallback and confirmed delete.
  No pretend visibility controls or permanent URLs.

## Evidence and remaining checks

Tests exercise the actual native addon/bridge against local HTTP, all SDK methods,
u64 precision and structured errors. Routing fixtures cover duplicate labels,
partial failures, cancellation, late native events and recovery races. Actual
Vencord normalization tests check receiver preservation, bypass recursion and
sidebar sibling syntax. Type/build checks use the pinned Vencord tree.

No running Discord is available. Public app fetching returned HTTP 403. Live
tests must establish active composer refresh, draft types, attachment cleanup,
large-file validation before this hook, current sidebar matching and pane layout.
The pane has no URL route; paths bypassing `uploadFiles` are outside this prototype.
Stable public links require the separate visibility architecture.
