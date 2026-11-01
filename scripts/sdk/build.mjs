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
const profile = process.argv.includes("--release") ? "sdk" : "dev";
const profileDir = profile === "dev" ? "debug" : profile;
const targetDir = resolve(process.env.CARGO_TARGET_DIR || "target");
const env = {
  ...process.env,
  CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS || "2",
  CARGO_TARGET_DIR: targetDir,
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
const privacyDeclarations = `
export interface PrivacyConfig {
  maxInputBytes?: bigint; maxOutputBytes?: bigint; maxInflatedBytes?: bigint;
  maxEntryBytes?: bigint; maxEntries?: number;
}
export interface PrivacyOptions { stripMetadata?: boolean; anonymizeFilename?: boolean; passphrase?: string; }
export interface PrivacyReport {
  policyVersion: string; metadataStripped: boolean; filenameAnonymized: boolean;
  encrypted: boolean; changed: boolean; retained: string[];
}
export interface PrivacyAssessment { canAttempt: boolean; format: string | null; reason: string | null; }
export interface PreparedFile { data: Uint8Array; filename: string; mimeType: string; sizeBytes: bigint; report: PrivacyReport; }
export interface NativeMediaPaths { ffmpegPath:string; ffprobePath:string; }
export interface MediaBackend { strip(data:Uint8Array,config?:PrivacyConfig,control?:{signal?:AbortSignal}):Promise<{data:Uint8Array;mimeType:string;retained:string[]}>; }
export class MetadataSanitizer {
  constructor(config?: PrivacyConfig, mediaBackend?: MediaBackend);
  assess(source: Uint8Array | Blob): Promise<PrivacyAssessment>;
  canStripMetadata(source: Uint8Array | Blob): Promise<boolean>;
  prepare(source: Uint8Array | Blob, filename: string, mimeType: string, options?: PrivacyOptions, control?: {signal?: AbortSignal}): Promise<PreparedFile>;
  strip(source: Uint8Array | Blob, filename?: string, mimeType?: string, control?: {signal?: AbortSignal}): Promise<PreparedFile>;
  decrypt(source: Uint8Array | Blob, passphrase: string, control?: {signal?: AbortSignal}): Promise<Uint8Array>;
}
`;
const declarations =
  types.stdout +
  privacyDeclarations +
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
  uploadSanitized(request: PresignRequest, source:Uint8Array|Blob, options?:PrivacyOptions, config?:PrivacyConfig, mediaBackend?:MediaBackend):Promise<{upload:UploadResult;privacy:PrivacyReport}>;
}
`;
run("bun", [
  "x",
  "--no-install",
  "napi",
  "build",
  "--manifest-path",
  "crates/condite-sdk-node/Cargo.toml",
  "--package-json-path",
  "crates/condite-sdk-node/package.json",
  "--output-dir",
  "crates/condite-sdk-node/dist",
  ...(profile === "dev" ? [] : ["--profile", profile]),
  "--dts",
  "native.d.ts",
]);
const nodeDir = "crates/condite-sdk-node/dist";
copyFileSync("crates/condite-sdk-node/index.cjs", `${nodeDir}/index.cjs`);
copyFileSync("crates/condite-sdk-node/LICENSE", `${nodeDir}/LICENSE`);
const nodeDeclarations = declarations
  .replaceAll("mediaBackend?: MediaBackend", "mediaPaths?: NativeMediaPaths")
  .replaceAll("mediaBackend?:MediaBackend", "mediaPaths?:NativeMediaPaths")
  .replaceAll(", control?: {signal?: AbortSignal}", "")
  .replace(
    "  assess(source:",
    "  prepareFile(path:string, options?:PrivacyOptions):Promise<PreparedFile>;\n  assess(source:",
  );
writeFileSync(`${nodeDir}/index.d.cts`, nodeDeclarations);
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
  profile,
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
  resolve(
    targetDir,
    "wasm32-unknown-unknown",
    profileDir,
    "condite_sdk_wasm.wasm",
  ),
]);
// Replace untyped JsValue declarations with the generated public contract.
writeFileSync(
  `${wasmDir}/index.d.ts`,
  declarations +
    '\nexport function createFfmpegBackend<T>(ffmpeg:T, options?:{timeoutMs?:number;legacyProbeExitCode?:boolean;reload?:(ffmpeg:T)=>Promise<unknown>}):MediaBackend;\nexport {default} from "./condite_sdk.js";\n',
);
writeFileSync(
  `${wasmDir}/index.js`,
  'export * from "./privacy.js"; export {default} from "./privacy.js";\n',
);
copyFileSync("crates/condite-sdk-wasm/privacy.js", `${wasmDir}/privacy.js`);
copyFileSync(
  "crates/condite-sdk-wasm/privacy-worker.js",
  `${wasmDir}/privacy-worker.js`,
);
copyFileSync(
  "crates/condite-sdk-wasm/ffmpeg-backend.js",
  `${wasmDir}/ffmpeg-backend.js`,
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
if (profile === "sdk") {
  run("cargo", [
    "build",
    "--locked",
    "-p",
    "condite-sdk",
    "--example",
    "health",
    "--profile",
    profile,
  ]);
  const report = {
    target: process.arch + "-" + process.platform,
    rustc: spawnSync("rustc", ["--version"], {
      encoding: "utf8",
    }).stdout.trim(),
    profile,
    bytes: {
      nativeAddon: statSync(`${nodeDir}/condite-sdk.node`).size,
      wasm: statSync(`${wasmDir}/condite_sdk_bg.wasm`).size,
      wasmGzip: gzipSync(readFileSync(`${wasmDir}/condite_sdk_bg.wasm`)).length,
      wasmGlue: statSync(`${wasmDir}/condite_sdk.js`).size,
      privacyFacade: statSync(`${wasmDir}/privacy.js`).size,
      privacyWorker: statSync(`${wasmDir}/privacy-worker.js`).size,
      mediaAdapter: statSync(`${wasmDir}/ffmpeg-backend.js`).size,
      rustExample: statSync(resolve(targetDir, profileDir, "examples/health"))
        .size,
    },
  };
  mkdirSync("target", { recursive: true });
  writeFileSync(
    "target/sdk-sizes.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(report);
}
