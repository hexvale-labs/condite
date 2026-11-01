# Condite for Vencord

Desktop custom userplugin: the message composer’s **Condite files** button opens
an extensible Condite submenu. **Upload file with privacy options** offers
metadata removal, filename anonymization and passphrase encryption; each starts
off for every operation. The SDK cleans, renames, then encrypts locally before
upload. A preparation failure aborts the upload. Ordinary Discord attachments
are unchanged, and this plugin never sends a message.

Completed uploads show **Copy share link**, expiry and the cleaning report.
The selectable link remains available if clipboard access fails. The most recent
result stays in memory under **Last share link** until the plugin stops.
Links are unsigned direct S3 bearer URLs; anyone holding one can download it. An encrypted
file still needs its passphrase after downloading; share that separately.
**Decrypt local file** opens a local file picker, requests the passphrase and
saves a decrypted copy to a location you choose. It makes no network request.
The encrypted original remains intact; the decrypted filename defaults to
`decrypted-file` because the encrypted payload does not carry its original name.

Set the API key in Condite settings; Vencord stores it locally. Alternatively,
provide `CONDITE_API_KEY` and `CONDITE_API_URL` in the desktop launch environment.
The environment URL must match the plugin setting before its key is used. Keys
are sent only to the configured API, never to signed storage uploads.

Retention defaults to 14 days. The privacy-options menu supports day intervals,
a UTC date or Never alongside encryption and metadata cleaning. Change the
persistent default in plugin settings. **Manage uploads and shared links** offers
listing, health checks, pending confirmation, owner downloads, deletion, independent
share creation/listing/revocation and retention changes, including adding/removing
intervals. Retention changes to a past date require deletion confirmation.

Each completed upload creates a direct share following its retention; failures
retain the confirmed private upload ID for recovery. Condite removes expired
uploads and share copies about every minute while online. Downtime or storage
failures can delay removal; downloaded or cached copies cannot be recalled.
Both integrations link the [AI notice](../../AI-NOTICE.md) in settings/about.

## Install

This requires a source-built desktop Vencord and a locally built Condite Node
SDK for the desktop’s operating system and architecture. It does not run in
browser Vencord, mobile Discord or a browser extension. The independent browser
SDK is not bundled here; a browser integration needs separate worker/assets and
Discord CSP qualification.

1. Follow [Vencord’s source/custom-plugin instructions](https://docs.vencord.dev/installing/custom-plugins/).
2. From the Condite repository, build the SDK with `bun run build:sdk`.
3. Copy this directory’s `condite.desktop` folder into your Vencord checkout’s
   `src/userplugins/`. Keep all source files, generated `wire.d.ts` and stylesheet together.
4. In the Vencord checkout run `pnpm install --frozen-lockfile`, `pnpm build`, then
   `pnpm inject`. Fully quit and restart Discord after native code changes.
5. Launch Discord with `CONDITE_SDK_NODE_PATH` set to the absolute path of your
   trusted local `crates/condite-sdk-node/dist/index.cjs`. For example on Linux:

   ```sh
   CONDITE_SDK_NODE_PATH=/opt/condite/crates/condite-sdk-node/dist/index.cjs discord
   ```

6. Enable **Condite** in Vencord’s plugin settings and set its API base URL. Use
   HTTPS for remote servers; localhost HTTP is available for development. If the
   composer icon is hidden, enable Condite in Vencord’s chat-bar button settings.

The SDK path comes from the desktop launch environment rather than renderer IPC;
the plugin cannot load a renderer-supplied module path. Nothing is downloaded
automatically. Optional media cleaning also requires explicitly installed local
FFmpeg/ffprobe executables: set both `CONDITE_FFMPEG_PATH` and
`CONDITE_FFPROBE_PATH` to their absolute paths before starting Discord. Without
those, media metadata cleaning fails before upload.

Passphrases are masked, confirmed for encryption, cleared from fields when the
operation begins, and never written to settings or logs. JavaScript strings and
IPC copies cannot promise secure memory erasure. The plugin processes one file
at a time, limits actual input reads to 300 MiB and prepared uploads/decrypted
output to 100 MiB. A running native operation has no cancellation interface;
closing its modal does not cancel processing. Failed uploads can leave a pending
server record. Supported formats and retained-content limits follow the
[SDK documentation](../../crates/condite-sdk/README.md).

## Checks

From the Condite root: `bun test clients/vencord/tests`. After building the SDK,
include the real native SDK / local HTTP fixture test with:

```sh
CONDITE_REAL_SDK_PATH="$PWD/crates/condite-sdk-node/dist/index.cjs" bun test clients/vencord/tests
```
After copying the plugin into a Vencord checkout:

```sh
pnpm test:ts
pnpm exec eslint src/userplugins/condite.desktop
pnpm build --standalone
```

Validated against Vencord `8d64c8be7e68616f8f116a6fff9480051aa2a0c5`
(v1.15.15), using its current
[ChatButtons API](https://github.com/Vendicated/Vencord/blob/8d64c8be7e68616f8f116a6fff9480051aa2a0c5/src/api/ChatButtons.tsx),
[modal types](https://github.com/Vendicated/Vencord/blob/8d64c8be7e68616f8f116a6fff9480051aa2a0c5/packages/discord-types/src/components/Modal.d.ts)
and [native bridge](https://docs.vencord.dev/plugins/native/).
An authenticated Discord session is still needed to verify compositor rendering,
keyboard navigation, native dialogs and clipboard behavior end to end. No live
Discord upload or message was performed during development.

## License

The plugin and tests are GPL-3.0-or-later. They use Vencord APIs and dynamically
load the separate MIT-licensed Condite Node SDK. No Vencord implementation was
copied into this directory. See [COPYING](COPYING).
