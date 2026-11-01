# Condite Vencord userplugin

Initial desktop integration using the Node/Electron Condite SDK in Vencord's
main-process bridge. The original blocking/toast hook was user-verified; the
expanded routing, editor updates and navigation need live-client testing.

## Build and install

Requires Git, Node 22+, global pnpm 12.15.0, Rust and a native C toolchain.
From this directory:

```sh
pnpm setup      # Shallow-fetch pinned Vencord, install dependencies, copy source
pnpm build      # Build Vencord AND the native SDK for this machine
pnpm check      # Vencord TypeScript and plugin lint checks
pnpm test       # Rebuild native SDK, then run fixtures with four Node workers
pnpm --dir .vencord inject
```

Select your desktop Discord installation in the installer. Fully quit and restart
Discord after installation or native changes; a renderer reload does not reload
Electron helpers. Enable **Condite** and set **API base** and **API key** in its settings.
The key is stored locally by Vencord and sent only to the configured API.

Output is **`.vencord/dist/`**. Keep **`condite-sdk/`** beside the main bundles when
moving the build. The addon is OS/architecture-specific; build on the target
machine. For Vesktop, select this custom Vencord build using its configuration
and retain that folder. The SDK remains MIT; the plugin follows Vencord's GPL.

`pnpm build:web` compiles the UI but cannot run the Electron SDK. Use desktop
Discord for this integration. `pnpm sdk:build` rebuilds only the addon.

Edit `src/`, then rebuild. `pnpm sync` refreshes the generated userplugin copy.
For `pnpm --dir .vencord watch`, sync after edits; native changes also require
rebuilding the SDK/restarting Discord. Vencord is pinned in `vencord.json`; move
aside the ignored checkout before setting up a different revision. Never commit
`.vencord`, native packages or build outputs.

## Behavior

- **Always Condite:** add links to the originating channel's unsent draft.
  Multiple files retain their order; duplicate labels use `filename (2)`, etc.
  Existing draft text is retained.
- **Automatic:** decide per file using Discord's current account/server limit.
  Files within the limit use Discord; larger files use Condite.
- **Choose for each file:** one file gets two buttons; batches get individual
  selectors. Closing the chooser preserves attachments.
- **Discord failures:** upload error events/rejected batch promises offer a
  Condite retry. Declining preserves attachments; known successes/cancellations
  are excluded, including completions while the prompt is open.

Update fallback limits in [src/limits.ts](src/limits.ts), or use **Discord limit
override in MiB** without rebuilding. A live named Discord limit helper is
preferred when available. **Condite itself currently caps files at 100 MiB**;
this plugin does not increase that cap.

**Uploads** appears beside Friends in the Home sidebar and opens a full React
content pane. It provides list/grid views, page-local search/status filters,
pagination, uploads/progress, health checks, pending confirmation, owner downloads,
Share/Manage links, retention updates and deletion confirmation. Toolbox/settings actions provide access if the sidebar
patch changes. Escape, channel navigation or **Back to Discord** closes it. This
experimental pane uses Discord's `main` landmark and has no URL route.

Completed uploads automatically create an unsigned direct S3 share following the
file's retention. Retention defaults to 14 days; change the default in settings
or select a per-batch interval, exact UTC date or Never in Uploads. Existing files
support interval additions/subtractions, a new date and Never. A past date asks
for deletion confirmation. Share creates independent links; Manage links lists,
copies and revokes them individually. Owner Download is a separate temporary
signed S3 URL. Failed sharing retains the confirmed private upload ID for recovery.

Condite removes expired uploads/share copies approximately every minute while
online. Downtime or storage failures can delay deletion; cached/downloaded copies
cannot be recalled. See [visibility architecture](../../docs/file-visibility-architecture.md).

`pnpm check` generates an ignored TypeScript configuration selecting Vencord's
pinned Electron declarations, avoiding conflicts with the root SDK's Electron
package without disabling declaration checks. Lint excludes generated wire types
and shared MIT translations, preserving their own notices. Both integrations disclose their
[AI-assisted authorship](../../AI-NOTICE.md) in settings/about.

## First live-client checks

1. Upload two files with the same name. Check links after existing draft text,
   separate stored files, and attachment removal only after success.
2. Switch channels/edit drafts while uploading. Verify links only enter the
   initiating channel and its active composer refreshes.
3. Test automatic routing with small/over-Discord-limit files below 100 MiB, and
   mixed chooser batches. Some builds may validate large files before this hook.
4. Interrupt a Discord upload; accept/decline recovery. Interrupt Condite PUT or
   confirm; check errors and pending recovery.
5. Open Uploads, change views/filter/pages, upload, generate a link, confirm a
   pending record, and cancel/confirm deletion.

The hook has no verified draft-type argument: this prototype targets regular
channel composers. Edit/voice/scheduled uploads and paths bypassing `uploadFiles`
need investigation. No automatic send action is added. Fixtures cannot establish
Discord caller behavior or live composer synchronization.

See [implementation notes](../../docs/superpowers/specs/2026-10-31-vencord-integration.md)
for research and remaining checks. Source is copied into a pinned ignored checkout
because Vencord scans `src/userplugins` without an external-directory option;
this avoids symlink resolution/permissions and fork/submodule maintenance.

## Language

Condite follows Discord's selected language, including changes while its pane is
open. Its UI shares the dashboard's [translation catalog](../i18n/README.md).
Unsupported languages fall back to English. This localizes Condite, not Vesktop's
own settings. The [official installation guide](https://docs.vencord.dev/installing/)
documents selecting a custom Vencord `dist` directory in Vesktop's developer
settings. Live Vesktop behavior still needs verification.
