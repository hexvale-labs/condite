import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile, copyFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

test("browser remux output may grow within output budget without changing input policy", async () => {
  const directory = await mkdtemp(join(tmpdir(), "condite-wrapper-"));
  const originalWorker = globalThis.Worker;
  try {
    await writeFile(join(directory, "package.json"), '{"type":"module"}');
    await writeFile(
      join(directory, "privacy.js"),
      await readFile(
        new URL("../../crates/condite-sdk-wasm/privacy.js", import.meta.url),
      ),
    );
    await copyFile(
      new URL(
        "../../crates/condite-sdk-wasm/ffmpeg-backend.js",
        import.meta.url,
      ),
      join(directory, "ffmpeg-backend.js"),
    );
    await writeFile(
      join(directory, "condite_sdk.js"),
      `
      export default async function init() {}
      export class Client {}
      export function privacyAssess(size, header, config) {
        return {canAttempt:true,reason:size>(config.maxInputBytes??314572800n)?'too-large':null,format:header.length?'mp3':null};
      }
      export function privacyMediaPlan() {}
      export function privacyValidateProbe() {}
    `,
    );
    globalThis.Worker = class {
      terminate() {}
      postMessage({ source, args, config }) {
        assert(
          BigInt(source.length) <= config.maxInputBytes,
          "worker receives stage input budget",
        );
        queueMicrotask(() =>
          this.onmessage({
            data: {
              result: {
                data: source,
                filename: args.filename,
                mimeType: args.mimeType,
                sizeBytes: BigInt(source.length),
                report: {
                  metadataStripped: false,
                  changed: false,
                  retained: [],
                },
              },
            },
          }),
        );
      }
    };
    const { MetadataSanitizer } = await import(
      pathToFileURL(join(directory, "privacy.js"))
    );
    const limits = { maxInputBytes: 3n, maxOutputBytes: 10n };
    const sanitizer = new MetadataSanitizer(limits, {
      strip: async () => ({
        data: new Uint8Array([1, 2, 3, 4]),
        mimeType: "audio/mpeg",
        retained: ["codec-bitstream-metadata-not-cleaned"],
      }),
    });
    const output = await sanitizer.prepare(
      new Uint8Array([73, 68, 51]),
      "a.mp3",
      "audio/mpeg",
      { stripMetadata: true },
    );
    assert.equal(output.sizeBytes, 4n);
    assert.equal(output.report.metadataStripped, true);
    assert.equal(limits.maxInputBytes, 3n);
    const idempotent = new MetadataSanitizer(limits, {
      strip: async (bytes) => ({
        data: bytes,
        mimeType: "audio/mpeg",
        retained: [],
      }),
    });
    const identical = await idempotent.strip(
      new Uint8Array([73, 68, 51]),
      "a.mp3",
    );
    assert.equal(
      identical.report.changed,
      false,
      "byte-identical remux must report unchanged",
    );
  } finally {
    globalThis.Worker = originalWorker;
    await rm(directory, { recursive: true, force: true });
  }
});

test("privacy worker preserves fixed Rust input codes and hides unexpected details", async () => {
  const directory = await mkdtemp(join(tmpdir(), "condite-worker-"));
  const originalSelf = globalThis.self;
  try {
    await writeFile(join(directory, "package.json"), '{"type":"module"}');
    await copyFile(
      new URL(
        "../../crates/condite-sdk-wasm/privacy-worker.js",
        import.meta.url,
      ),
      join(directory, "worker.js"),
    );
    await writeFile(
      join(directory, "condite_sdk.js"),
      `
      export default async function init() {}
      export function privacyPrepare(_data,_name,_mime,options) {throw new Error(options.message);}
      export function privacyDecrypt() {}
    `,
    );
    let reply;
    globalThis.self = {
      postMessage: (value) => {
        reply = value;
      },
    };
    await import(pathToFileURL(join(directory, "worker.js")));
    const request = (message) => ({
      data: {
        operation: "prepare",
        source: new Uint8Array([1]),
        args: { filename: "a", mimeType: "text/plain", options: { message } },
        config: {},
      },
    });
    await globalThis.self.onmessage(
      request("invalid input: unsupported-format"),
    );
    assert.equal(reply.error, "unsupported-format");
    await globalThis.self.onmessage(
      request("unexpected secret exception details"),
    );
    assert.equal(reply.error, "processing-failed");
  } finally {
    globalThis.self = originalSelf;
    await rm(directory, { recursive: true, force: true });
  }
});
