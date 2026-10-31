import { spawnSync } from "node:child_process";
import { cpSync, existsSync, lstatSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL(".", import.meta.url));
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
  writeFileSync(marker, "Managed by clients/vencord-extension/tooling.mjs\n");
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
  } else if (["sync", "check", "build", "build:web"].includes(action)) {
    sync();
    if (action === "check") run(pnpm, ["test:ts"]);
    if (action === "build") run(pnpm, ["build"]);
    if (action === "build:web") run(pnpm, ["buildWeb"]);
  } else {
    throw new Error("Usage: node tooling.mjs setup|sync|check|build|build:web");
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
