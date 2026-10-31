# Condite Vencord userplugin

Upload interception PoC: `Condite` patches `async uploadFiles(`, shows the batch's
filenames in a toast, and returns before Discord's upload code runs. It does not
upload to Condite or insert links yet. The patch still needs live-client testing.

Enable Condite, reload Discord, and attach a file. When Discord calls the patched
function, expect `Condite PoC blocked upload: <filename>` and no Discord upload
from that call. Attachment UI may remain pending because this PoC intentionally
skips queue cleanup. Disable Condite and reload to restore normal uploads.

## Development

Requires Git, Node.js 22+, and global pnpm 12.15.0 (`npm install -g pnpm@12.15.0`).
Run these commands in this directory:

```sh
pnpm setup      # Fetch pinned Vencord, install its locked dependencies, copy plugin
pnpm check      # Copy plugin and run Vencord's TypeScript check
pnpm build      # Copy plugin and build desktop Vencord
pnpm build:web  # Copy plugin and build browser Vencord
```

Edit `src/`, then rebuild. `.vencord/src/userplugins/condite` is a generated copy;
edits there are overwritten. Artifacts and upstream dependencies remain inside
the ignored `.vencord/` directory. Setup/build do not inject into Discord.
Follow [Vencord's installation instructions](https://docs.vencord.dev/installing/)
for installing a local build, then enable Condite in the plugin settings.

For upstream watch mode, run `pnpm --dir .vencord watch` and run `pnpm sync`
after editing our source. Reload Discord after rebuilding patches.

The upstream revision is recorded in `vencord.json`; pnpm is pinned in
`package.json`. To upgrade, update both pins as needed and move aside the old
`.vencord` checkout before setup. Keep any local work first. Root Condite npm
tooling remains independent; do not install Vencord dependencies at the root.

## Why a copied userplugin?

[Vencord requires custom plugins to be compiled into a source build](https://docs.vencord.dev/installing/custom-plugins/).
Its [plugin discovery](https://github.com/Vendicated/Vencord/blob/8d64c8be7e68616f8f116a6fff9480051aa2a0c5/scripts/build/common.mjs#L150)
scans `src/plugins` and `src/userplugins`; it has no external source-directory
option. A separately bundled npm package is not a runtime plugin installation.

- **Pinned ignored checkout + copy (chosen):** ordinary files resolve Vencord's
  aliases and dependencies inside its own tree; no symlink permissions or
  submodule initialization. Copying is automatic before checks/builds.
- **Symlink:** fewer copies and convenient watching, but resolving imports outside
  Vencord's tree and Windows permissions need care.
- **Submodule/fork:** useful for upstream modifications, but unnecessary Git
  lifecycle overhead when only adding a userplugin.

## Upload investigation

Upstream's [AnonymiseFileNames patch](https://github.com/Vendicated/Vencord/blob/8d64c8be7e68616f8f116a6fff9480051aa2a0c5/src/plugins/anonymiseFileNames/index.tsx)
finds `async uploadFiles(`. This PoC uses that same match with an unconditional
early return. It is not a verified interception contract: inspect the running
client's module, parameters, return value, cancellation, and attachment queue
before implementing upload redirection. Other upload paths may bypass this hook.

[`insertTextIntoChatInputBox`](https://github.com/Vendicated/Vencord/blob/8d64c8be7e68616f8f116a6fff9480051aa2a0c5/src/utils/discord.tsx#L110)
dispatches `INSERT_TEXT` to the last subscribed composer; it has no channel-ID
argument. Capture the initiating channel before awaiting uploads. Investigate
channel-specific drafts (`DraftStore` and the `saveDraft` action) so switching
channels cannot paste into another conversation. Insert into the draft; do not
automatically send a message or replace existing text.

Condite's current flow is presign → storage PUT → confirm. Current download links
expire; stable public links are still a proposal. Follow the
[visibility architecture](../../docs/file-visibility-architecture.md) before
implementing sharing. No upload transport or SDK dependency has been selected;
Vencord is GPL-3.0-or-later, and SDK/core license boundaries must remain intact.
