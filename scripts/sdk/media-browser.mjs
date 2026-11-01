// Optional qualification: configure local @ffmpeg/ffmpeg and @ffmpeg/core modules
// plus native binaries for independent output decoding. No downloads occur here.
import assert from "node:assert/strict";
import http from "node:http";
import { readFile, mkdtemp, writeFile, rm } from "node:fs/promises";
import { resolve, extname, join } from "node:path";
import { tmpdir } from "node:os";
import { spawnSync } from "node:child_process";
import { chromium } from "@playwright/test";

const root = resolve(import.meta.dirname, "../..");
const modules = process.env.CONDITE_FFMPEG_BROWSER_MODULES;
const ffmpeg = process.env.CONDITE_TEST_FFMPEG;
const ffprobe = process.env.CONDITE_TEST_FFPROBE;
assert(
  modules && ffmpeg && ffprobe,
  "Set CONDITE_FFMPEG_BROWSER_MODULES, CONDITE_TEST_FFMPEG and CONDITE_TEST_FFPROBE",
);
const fixtures = JSON.parse(
  await readFile(join(root, "scripts/sdk/fixtures/media.json"), "utf8"),
);
const server = http.createServer(async (req, res) => {
  res.setHeader("Cross-Origin-Opener-Policy", "same-origin");
  res.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
  try {
    const pathname = new URL(req.url, "http://localhost").pathname;
    if (pathname === "/") {
      res.setHeader("Content-Type", "text/html");
      res.end("<!doctype html><title>Media qualification</title>");
      return;
    }
    let path;
    if (pathname.startsWith("/ffmpeg/"))
      path = resolve(modules, "@ffmpeg/ffmpeg/dist/esm", pathname.slice(8));
    else if (pathname.startsWith("/core/"))
      path = resolve(modules, "@ffmpeg/core/dist/esm", pathname.slice(6));
    else if (pathname.startsWith("/sdk/"))
      path = resolve(root, "crates/condite-sdk-wasm/dist", pathname.slice(5));
    else if (pathname === "/adapter.js")
      path = join(root, "crates/condite-sdk-wasm/ffmpeg-backend.js");
    else throw new Error("unknown path");
    assert(
      path.startsWith(root + "/") || path.startsWith(resolve(modules) + "/"),
    );
    res.setHeader(
      "Content-Type",
      extname(path) === ".wasm" ? "application/wasm" : "text/javascript",
    );
    res.end(await readFile(path));
  } catch {
    res.writeHead(404).end();
  }
});
await new Promise((done) => server.listen(0, "127.0.0.1", done));
const browser = await chromium.launch({
  headless: true,
  args: ["--no-sandbox"],
});
const directory = await mkdtemp(join(tmpdir(), "condite-browser-media-"));
try {
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  const results = await page.evaluate(async (fixtures) => {
    const { FFmpeg } = await import("/ffmpeg/index.js");
    const { createFfmpegBackend: rawFactory } =
      await import("/sdk/ffmpeg-backend.js");
    const {
      mediaPlan,
      validateMediaProbe,
      createFfmpegBackend,
      MetadataSanitizer,
    } = await import("/sdk/privacy.js");
    const ffmpeg = new FFmpeg();
    const loadConfig = {
      coreURL: new URL("/core/ffmpeg-core.js", location.href).href,
      wasmURL: new URL("/core/ffmpeg-core.wasm", location.href).href,
    };
    await ffmpeg.load(loadConfig);
    const backend = createFfmpegBackend(ffmpeg, {
      legacyProbeExitCode: true,
      reload: (instance) => instance.load(loadConfig),
    });
    const sanitizer = new MetadataSanitizer({}, backend);
    const results = {};
    try {
      for (const [filename, fixture] of Object.entries(fixtures)) {
        const input = Uint8Array.from(atob(fixture.input), (v) =>
          v.charCodeAt(0),
        );
        const bytesBefore = input.slice();
        const result = await sanitizer.strip(input, filename);
        if (
          result.sizeBytes !== BigInt(result.data.byteLength) ||
          !result.report.metadataStripped
        )
          throw new Error("invalid sanitized result");
        if (
          input.byteLength !== bytesBefore.byteLength ||
          input.some((v, i) => v !== bytesBefore[i])
        )
          throw new Error("original input changed");
        if (new TextDecoder().decode(result.data).includes("PRIVATE_CANARY"))
          throw new Error(`metadata retained ${filename}`);
        if (
          !result.report.retained.includes(
            "codec-bitstream-metadata-not-cleaned",
          )
        )
          throw new Error("missing bitstream limitation");
        results[filename] = Array.from(result.data);
      }
      const controller = new AbortController();
      const deferredPlan = async (data) => {
        const plan = await mediaPlan(data);
        controller.abort();
        return plan;
      };
      let rejected = false;
      try {
        const input = Uint8Array.from(
          atob(Object.values(fixtures)[0].input),
          (v) => v.charCodeAt(0),
        );
        await rawFactory(ffmpeg, deferredPlan, validateMediaProbe).strip(
          input,
          {},
          { signal: controller.signal },
        );
      } catch (error) {
        rejected = error.name === "AbortError";
      }
      if (!rejected) throw new Error("abort during plan did not reject");
      if (
        (await ffmpeg.listDir("/")).some((entry) =>
          entry.name.startsWith("condite-"),
        )
      )
        throw new Error("virtual files leaked");
      return results;
    } finally {
      ffmpeg.terminate();
    }
  }, fixtures);
  const run = (executable, args) => {
    const result = spawnSync(executable, args, { maxBuffer: 16 * 1024 * 1024 });
    assert.equal(result.status, 0, result.stderr?.toString());
    return result.stdout;
  };
  for (const [filename, fixture] of Object.entries(fixtures)) {
    const input = join(directory, `input-${filename}`);
    const output = join(directory, `output-${filename}`);
    await writeFile(input, Buffer.from(fixture.input, "base64"));
    await writeFile(output, Buffer.from(results[filename]));
    const decode = (path) =>
      run(ffmpeg, [
        "-v",
        "error",
        "-nostdin",
        "-noautorotate",
        "-i",
        path,
        ...(fixture.kind === "audio"
          ? ["-map", "0:a:0", "-f", "s16le", "-acodec", "pcm_s16le"]
          : ["-map", "0:v:0", "-f", "rawvideo", "-pix_fmt", "rgb24"]),
        "pipe:1",
      ]);
    assert.deepEqual(
      decode(output),
      decode(input),
      `decoded ${filename} changed`,
    );
    if (fixture.kind === "video") {
      const inspect = (path) =>
        JSON.parse(
          run(ffprobe, ["-v", "error", "-show_streams", "-of", "json", path]),
        ).streams[0];
      const before = inspect(input);
      const after = inspect(output);
      for (const field of [
        "side_data_list",
        "color_space",
        "color_primaries",
        "color_transfer",
        "color_range",
      ])
        assert.deepEqual(after[field], before[field], `${filename} ${field}`);
    }
    console.log(
      `Browser FFmpeg: ${filename} metadata removed; decoded content preserved`,
    );
  }
} finally {
  await browser.close();
  await new Promise((done) => server.close(done));
  await rm(directory, { recursive: true, force: true });
}
