import http from "node:http";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve, extname } from "node:path";
import { createRequire } from "node:module";
import { spawn } from "node:child_process";
import { chromium } from "@playwright/test";
import { contract } from "./contract.mjs";
const require = createRequire(import.meta.url);
const root = resolve(import.meta.dirname, "../..");
const calls = [];
const server = http.createServer(async (req, res) => {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Headers", "content-type");
  res.setHeader(
    "Access-Control-Allow-Methods",
    "GET, POST, PUT, DELETE, OPTIONS",
  );
  if (req.method === "OPTIONS") {
    res.writeHead(204).end();
    return;
  }
  calls.push(`${req.method} ${req.url}`);
  try {
    const url = new URL(req.url, "http://localhost");
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    const bytes = Buffer.concat(chunks);
    if (url.pathname === "/contract.mjs" || url.pathname.startsWith("/wasm/")) {
      const path =
        url.pathname === "/contract.mjs"
          ? resolve(root, "scripts/sdk/contract.mjs")
          : resolve(
              root,
              "crates/condite-sdk-wasm/dist",
              url.pathname.slice(6),
            );
      assert(path.startsWith(root));
      res.setHeader(
        "content-type",
        extname(path) === ".wasm" ? "application/wasm" : "text/javascript",
      );
      res.end(await readFile(path));
      return;
    }
    const json = (value) => {
      res.setHeader("content-type", "application/json");
      res.end(typeof value === "string" ? value : JSON.stringify(value));
    };
    if (url.pathname === "/") {
      res.setHeader("content-type", "text/html");
      res.end("<!doctype html><title>SDK integration fixture</title>");
      return;
    }
    if (req.method === "GET" && url.pathname === "/prefix/v1/health")
      return json({
        healthy: true,
        started_at: "2026-10-31T00:00:00Z",
        runtime: "PT1S",
      });
    if (req.method === "GET" && url.pathname === "/prefix/v1/uploads") {
      assert.equal(url.searchParams.get("limit"), "2");
      assert.equal(url.searchParams.get("after"), "cursor&value");
      return json(
        '{"uploads":[{"id":"id","filename":"a.txt","mime_type":"text/plain","size_bytes":18446744073709551615,"status":"Confirmed","created_at":"2026-10-31T00:00:00Z"}],"next_after":null}',
      );
    }
    if (req.method === "POST" && url.pathname === "/prefix/v1/uploads") {
      const body = JSON.parse(bytes);
      assert.equal(body.size_bytes, 3);
      return json({
        request_id:
          body.filename === "fail.txt"
            ? "fail"
            : body.filename === "put-fail.txt"
              ? "put-fail"
              : "id",
        uri: `http://${req.headers.host}/${body.filename === "put-fail.txt" ? "storage-fail" : "storage"}`,
        headers: {
          "content-type": "text/plain",
          "content-length": "3",
          host: req.headers.host,
        },
        expires_at_utc: "later",
      });
    }
    if (req.method === "PUT" && url.pathname === "/storage-fail") {
      res.writeHead(503).end("storage unavailable");
      return;
    }
    if (req.method === "PUT" && url.pathname === "/storage") {
      assert.equal(bytes.toString(), "abc");
      assert.equal(req.headers["content-type"], "text/plain");
      assert.equal(req.headers.authorization, undefined);
      res.writeHead(200).end();
      return;
    }
    if (
      req.method === "POST" &&
      url.pathname === "/prefix/v1/uploads/id/confirm"
    )
      return json(
        '{"public_uri":"https://example.com/file","expires_at":"later","new_storage_size_bytes":18446744073709551615}',
      );
    if (req.method === "DELETE" && url.pathname === "/prefix/v1/uploads/id") {
      res.writeHead(204).end();
      return;
    }
    if (url.pathname === "/prefix/v1/uploads/fail/confirm") {
      res.writeHead(409).end("not ready");
      return;
    }
    res.writeHead(404).end("missing upload");
  } catch (error) {
    console.error(error);
    res.writeHead(500).end(String(error));
  }
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server.address().port}`;
try {
  if (process.env.SDK_TEST_TARGET !== "browser") {
    const { Client } = require(
      resolve(root, "crates/condite-sdk-node/dist/index.cjs"),
    );
    assert.equal(await contract(Client, base), "passed");
    console.log("Node contract passed");
    const electron = require("electron");
    const command =
      process.platform === "linux" && !process.env.DISPLAY
        ? "xvfb-run"
        : electron;
    const args = command === "xvfb-run" ? ["-a", electron] : [];
    args.push("--no-sandbox", resolve(root, "scripts/sdk/electron.cjs"), base);
    const electronEnv = { ...process.env };
    delete electronEnv.ELECTRON_RUN_AS_NODE;
    const proc = spawn(command, args, { env: electronEnv, stdio: "inherit" });
    assert.equal(
      await new Promise((r, j) => {
        proc.on("error", j);
        proc.on("exit", r);
      }),
      0,
      "Electron contract",
    );
  }
  const browser = await chromium.launch({
    headless: true,
    args: ["--no-sandbox"],
  });
  try {
    const page = await browser.newPage();
    await page.goto(base);
    assert.equal(
      await page.evaluate(async (base) => {
        const { default: init, Client } = await import("/wasm/index.js");
        await init();
        const { contract } = await import("/contract.mjs");
        return contract(Client, base);
      }, base),
      "passed",
    );
    console.log("Browser WASM contract passed");
  } finally {
    await browser.close();
  }
  assert(calls.some((c) => c === "PUT /storage"));
} finally {
  server.closeAllConnections();
  await new Promise((r) => server.close(r));
}
