// Used only by Electron's native plugin module, never by the renderer.
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { app, dialog } from "electron";
import { clientConfig } from "./config";

export function nativeConfig() {
  const file = join(app.getPath("userData"), "condite.toml");
  return existsSync(file)
    ? clientConfig(readFileSync(file, "utf8"), process.env)
    : {};
}
export async function importConfigFile() {
  const choice = await dialog.showOpenDialog({
    title: "Import Condite TOML",
    properties: ["openFile"],
    filters: [{ name: "TOML", extensions: ["toml"] }],
  });
  if (choice.canceled || choice.filePaths.length !== 1) return null;
  // Import only UI preferences. Native executable paths remain trusted disk configuration.
  const {
    sdk_path: _sdk,
    ffmpeg_path: _ffmpeg,
    ffprobe_path: _ffprobe,
    ...preferences
  } = clientConfig(readFileSync(choice.filePaths[0], "utf8"), process.env);
  return preferences;
}
