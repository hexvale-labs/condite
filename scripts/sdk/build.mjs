import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  statSync,
} from "node:fs";
import { gzipSync } from "node:zlib";
import { resolve } from "node:path";
const root = resolve(import.meta.dirname, "../..");
process.chdir(root);
const env = {
  ...process.env,
  CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS || "2",
  CARGO_TARGET_DIR: resolve("target/sdk"),
};
function run(cmd, args) {
  const p = spawnSync(cmd, args, { env, stdio: "inherit" });
  if (p.status !== 0)
    throw new Error(`${cmd} failed (${p.status}): ${p.error || ""}`);
}
const types = spawnSync(
  "cargo",
  [
    "run",
    "--quiet",
    "--locked",
    "-p",
    "condite-shared",
    "--features",
    "typescript",
    "--example",
    "export-types",
  ],
  { env, encoding: "utf8" },
);
if (types.status !== 0) throw new Error(types.stderr);
if (
  types.stdout !== readFileSync("crates/condite-shared/bindings.d.ts", "utf8")
) {
  throw new Error(
    "Shared TypeScript declarations are stale; regenerate bindings.d.ts with export-types",
  );
}
const declarations =
  types.stdout +
  `
export interface SdkError extends Error {
  code: 'input' | 'transport' | 'http' | 'decode';
  status: number | null;
  body: string | null;
  request_id: string | null;
  stage: "put" | "confirm" | null;
}
export class Client {
  constructor(baseUrl: string);
  health(): Promise<HealthStatus>;
  presign(request: PresignRequest): Promise<PresignResponse>;
  listUploads(request?: ListUploadsRequest): Promise<ListUploadsResponse>;
  confirm(id: string): Promise<ConfirmResponse>;
  delete(id: string): Promise<void>;
  putUpload(signed: PresignResponse, data: Uint8Array): Promise<void>;
  upload(request: PresignRequest, data: Uint8Array): Promise<UploadResult>;
}
`;
run("npx", [
  "napi",
  "build",
  "--manifest-path",
  "crates/condite-sdk-node/Cargo.toml",
  "--package-json-path",
  "crates/condite-sdk-node/package.json",
  "--output-dir",
  "crates/condite-sdk-node/dist",
  "--profile",
  "sdk",
  "--dts",
  "native.d.ts",
]);
const nodeDir = "crates/condite-sdk-node/dist";
copyFileSync("crates/condite-sdk-node/index.cjs", `${nodeDir}/index.cjs`);
copyFileSync("crates/condite-sdk-node/LICENSE", `${nodeDir}/LICENSE`);
writeFileSync(`${nodeDir}/index.d.cts`, declarations);
writeFileSync(
  `${nodeDir}/package.json`,
  JSON.stringify(
    {
      name: "condite-sdk-node",
      version: "0.1.0",
      license: "MIT",
      main: "index.cjs",
      types: "index.d.cts",
    },
    null,
    2,
  ) + "\n",
);
run("cargo", [
  "build",
  "--locked",
  "-p",
  "condite-sdk-wasm",
  "--target",
  "wasm32-unknown-unknown",
  "--profile",
  "sdk",
]);
const wasmDir = "crates/condite-sdk-wasm/dist";
mkdirSync(wasmDir, { recursive: true });
const bindgen = process.env.WASM_BINDGEN || "wasm-bindgen";
run(bindgen, [
  "--target",
  "web",
  "--out-dir",
  wasmDir,
  "--out-name",
  "condite_sdk",
  "target/sdk/wasm32-unknown-unknown/sdk/condite_sdk_wasm.wasm",
]);
// Replace untyped JsValue declarations with the generated public contract.
writeFileSync(
  `${wasmDir}/index.d.ts`,
  declarations + '\nexport {default} from "./condite_sdk.js";\n',
);
writeFileSync(
  `${wasmDir}/index.js`,
  'export {default, Client} from "./condite_sdk.js";\n',
);
copyFileSync("crates/condite-sdk-wasm/LICENSE", `${wasmDir}/LICENSE`);
writeFileSync(
  `${wasmDir}/package.json`,
  JSON.stringify(
    {
      name: "condite-sdk-wasm",
      version: "0.1.0",
      license: "MIT",
      type: "module",
      main: "index.js",
      types: "index.d.ts",
    },
    null,
    2,
  ) + "\n",
);
run("cargo", [
  "build",
  "--locked",
  "-p",
  "condite-sdk",
  "--example",
  "health",
  "--profile",
  "sdk",
]);
const report = {
  target: process.arch + "-" + process.platform,
  rustc: spawnSync("rustc", ["--version"], { encoding: "utf8" }).stdout.trim(),
  profile: "sdk (opt-level=z, LTO, codegen-units=1, symbols stripped)",
  bytes: {
    nativeAddon: statSync(`${nodeDir}/condite-sdk.node`).size,
    wasm: statSync(`${wasmDir}/condite_sdk_bg.wasm`).size,
    wasmGzip: gzipSync(readFileSync(`${wasmDir}/condite_sdk_bg.wasm`)).length,
    wasmGlue: statSync(`${wasmDir}/condite_sdk.js`).size,
    rustExample: statSync("target/sdk/sdk/examples/health").size,
  },
};
writeFileSync("target/sdk-sizes.json", JSON.stringify(report, null, 2) + "\n");
console.log(report);
