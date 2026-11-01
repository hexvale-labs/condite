import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, lstatSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL(".", import.meta.url));
const repository = fileURLToPath(new URL("../../", import.meta.url));
const checkout = join(root, ".vencord");
const target = join(checkout, "src/userplugins/condite");
const marker = join(target, ".condite-managed");
const pin = JSON.parse(readFileSync(join(root, "vencord.json"), "utf8"));
const pnpm = process.platform === "win32" ? "pnpm.cmd" : "pnpm";

function run(command, args, cwd = checkout) {
  const result = spawnSync(command, args, { cwd, stdio: "inherit", shell: process.platform === "win32" });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed (${result.status})`);
}

function verifyCheckout() {
  const result = spawnSync("git", ["rev-parse", "HEAD"], { cwd: checkout, encoding: "utf8" });
  if (result.status !== 0 || result.stdout.trim() !== pin.revision) {
    throw new Error("Vencord checkout is missing or differs from vencord.json. Run setup with a fresh .vencord directory.");
  }
}

function sync() {
  verifyCheckout();
  if (existsSync(target)) {
    if (lstatSync(target).isSymbolicLink() || !existsSync(marker)) {
      throw new Error("Refusing to replace an unmanaged condite userplugin.");
    }
    rmSync(target, { recursive: true });
  }
  cpSync(join(root, "src"), target, { recursive: true });
  cpSync(join(repository, "clients/i18n"), join(target, "translations"), { recursive: true, filter: source => !source.endsWith(".test.ts") });
  cpSync(join(repository, "crates/condite-shared/bindings.d.ts"), join(target, "wire.d.ts"));
  writeFileSync(marker, "Managed by clients/vencord-extension/tooling.mjs\n");
}

function checkTypes() {
  // Pin Electron imports to Vencord's own declarations instead of a parent
  // workspace's Electron package, without skipping declaration checks.
  const require = createRequire(join(checkout, "package.json"));
  const ts = require("typescript");
  const source = ts.readConfigFile(join(checkout, "tsconfig.json"), ts.sys.readFile);
  if (source.error) throw new Error(ts.flattenDiagnosticMessageText(source.error.messageText, "\n"));
  const config = join(checkout, "tsconfig.condite.json");
  writeFileSync(config, JSON.stringify({
    extends: "./tsconfig.json",
    compilerOptions: { paths: { ...source.config.compilerOptions.paths,
      electron: ["./node_modules/standalone-electron-types"] } }
  }, null, 2) + "\n");
  run(pnpm, ["exec", "tsc", "--noEmit", "--project", config]);
  // Generated wire types and MIT translations retain their own upstream notices.
  run(pnpm, ["exec", "eslint", target, "--ignore-pattern", "**/translations/**", "--ignore-pattern", "**/wire.d.ts"]);
}

function buildSdk() {
  run("cargo", ["build", "--locked", "-p", "condite-sdk-node", "--profile", "sdk", "--target-dir", join(repository, "target")], repository);
  const library = process.platform === "win32" ? "condite_sdk_node.dll"
    : process.platform === "darwin" ? "libcondite_sdk_node.dylib" : "libcondite_sdk_node.so";
  const destination = join(checkout, "dist/condite-sdk");
  mkdirSync(destination, { recursive: true });
  cpSync(join(repository, "target/sdk", library), join(destination, "condite-sdk.node"));
  cpSync(join(repository, "crates/condite-sdk-node/index.cjs"), join(destination, "index.cjs"));
  cpSync(join(repository, "crates/condite-sdk-node/LICENSE"), join(destination, "LICENSE"));
  writeFileSync(join(destination, "platform.json"), JSON.stringify({ platform: process.platform, arch: process.arch }) + "\n");
}

try {
  const action = process.argv[2];
  if (action === "setup") {
    if (!existsSync(checkout)) {
      mkdirSync(checkout);
      run("git", ["init"]);
      run("git", ["remote", "add", "origin", pin.repository]);
      run("git", ["fetch", "--depth", "1", "origin", pin.revision]);
      run("git", ["checkout", "--detach", "FETCH_HEAD"]);
    }
    verifyCheckout();
    run(pnpm, ["install", "--frozen-lockfile"]);
    sync();
  } else if (action === "sdk:build") {
    verifyCheckout();
    buildSdk();
  } else if (["sync", "check", "build", "build:web"].includes(action)) {
    sync();
    if (action === "check") checkTypes();
    if (action === "build") { run(pnpm, ["build"]); buildSdk(); }
    if (action === "build:web") run(pnpm, ["buildWeb"]);
  } else {
    throw new Error("Usage: node tooling.mjs setup|sync|check|build|build:web|sdk:build");
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
