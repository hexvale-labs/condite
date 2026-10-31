import { spawnSync } from "node:child_process";
for (const [pkg, target] of [
  ["condite-sdk", null],
  ["condite-sdk-wasm", "wasm32-unknown-unknown"],
]) {
  const args = [
    "tree",
    "--locked",
    "-p",
    pkg,
    "--edges",
    "normal",
    "--prefix",
    "none",
  ];
  if (target) args.push("--target", target);
  const result = spawnSync("cargo", args, { encoding: "utf8" });
  if (result.status !== 0) throw new Error(result.stderr);
  for (const dependency of [
    "condite-core",
    "aws-sdk",
    "toasty",
    "axum",
    "ts-rs",
  ]) {
    if (result.stdout.includes(dependency))
      throw new Error(`${pkg} includes ${dependency}`);
  }
  if (target && /rustls|tokio v|napi v/.test(result.stdout))
    throw new Error("Native runtime in browser dependency tree");
  console.log(`${pkg}: dependency isolation passed`);
}
