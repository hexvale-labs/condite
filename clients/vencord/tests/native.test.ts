// SPDX-License-Identifier: GPL-3.0-or-later
import { afterAll, beforeEach, expect, mock, test } from "bun:test";
import { chmod, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const folder = await mkdtemp(join(tmpdir(), "condite-vencord-test-"));
const inputPath = join(folder, "secret-name.txt");
const savePath = join(folder, "decrypted-copy");
const sdkPath = join(folder, "sdk.cjs");
let selected: { canceled: boolean; filePaths: string[]; };
let pickerCalls = 0;
let destination = savePath;
let trace: string[] = [];
let plain: Uint8Array | undefined;
const previousSdk = process.env.CONDITE_SDK_NODE_PATH;
const previousFfmpeg = process.env.CONDITE_FFMPEG_PATH;
const previousFfprobe = process.env.CONDITE_FFPROBE_PATH;

// Electron's dialogs and the separately loaded SDK are host boundaries. File I/O
// and the plugin's complete native handlers run unchanged.
mock.module("electron", () => ({ dialog: {
    async showOpenDialog() { pickerCalls++; return selected; },
    async showSaveDialog() { return { canceled: false, filePath: destination }; }
} }));
await writeFile(sdkPath, `module.exports = {
  MetadataSanitizer: class {
    async prepare(data, filename, mime, options) {
      globalThis.__conditeNativeTest.trace.push('prepare');
      return {data:new Uint8Array([9]),filename:'anonymous.txt',mimeType:'application/octet-stream',sizeBytes:1n,report:{policyVersion:'test',metadataStripped:false,filenameAnonymized:true,encrypted:false,changed:true,retained:[]}};
    }
    async decrypt() {
      globalThis.__conditeNativeTest.trace.push('decrypt');
      const plain = new Uint8Array([4,5,6]);
      globalThis.__conditeNativeTest.setPlain(plain);
      return plain;
    }
  },
  Client: class {
    async upload(request,data) {
      globalThis.__conditeNativeTest.trace.push('upload');
      return {confirmation:{public_uri:'https://storage.example/temporary',expires_at:'later'}};
    }
  }
};`);
const native = await import("../condite.desktop/native");
const event = {} as Parameters<typeof native.chooseAndUpload>[0];

beforeEach(async () => {
    process.env.CONDITE_SDK_NODE_PATH = sdkPath;
    delete process.env.CONDITE_FFMPEG_PATH;
    delete process.env.CONDITE_FFPROBE_PATH;
    trace = [];
    plain = undefined;
    (globalThis as any).__conditeNativeTest = { trace, setPlain: (bytes: Uint8Array) => { plain = bytes; } };
    pickerCalls = 0;
    destination = savePath;
    selected = { canceled: false, filePaths: [inputPath] };
    await writeFile(inputPath, new Uint8Array([1, 2, 3]));
});
afterAll(async () => {
    for (const [name, value] of [["CONDITE_SDK_NODE_PATH", previousSdk], ["CONDITE_FFMPEG_PATH", previousFfmpeg], ["CONDITE_FFPROBE_PATH", previousFfprobe]]) {
        if (value === undefined) delete process.env[name!]; else process.env[name!] = value;
    }
    delete (globalThis as any).__conditeNativeTest;
    await rm(folder, { recursive: true, force: true });
});

test("native picker uploads prepared file and clears transient passphrase reference", async () => {
    const options = { stripMetadata: false, anonymizeFilename: true, passphrase: "transient canary" };
    const reply = await native.chooseAndUpload(event, "https://condite.example", options);
    expect(reply.status).toBe("ok");
    expect(trace).toEqual(["prepare", "upload"]);
    expect(options.passphrase).toBeUndefined();
    expect(JSON.stringify(reply)).not.toContain("transient canary");
    expect(JSON.stringify(reply)).not.toContain("secret-name");
});

test("cancelled file picker makes no preparation or upload calls", async () => {
    selected = { canceled: true, filePaths: [] };
    expect(await native.chooseAndUpload(event, "https://condite.example", { stripMetadata: false, anonymizeFilename: false })).toEqual({ status: "cancelled" });
    expect(trace).toEqual([]);
});

test("SDK path is required before opening any file picker", async () => {
    delete process.env.CONDITE_SDK_NODE_PATH;
    const reply = await native.chooseAndUpload(event, "https://condite.example", { stripMetadata: false, anonymizeFilename: false });
    expect(reply.status).toBe("error");
    expect(pickerCalls).toBe(0);
    expect(trace).toEqual([]);
});

test("decrypt saves locally, retains encrypted original and clears plaintext buffer", async () => {
    expect(await native.chooseDecryptAndSave(event, "transient canary")).toEqual({ status: "ok" });
    expect(trace).toEqual(["decrypt"]);
    expect(new Uint8Array(await readFile(savePath))).toEqual(new Uint8Array([4, 5, 6]));
    expect(new Uint8Array(await readFile(inputPath))).toEqual(new Uint8Array([1, 2, 3]));
    expect(plain).toEqual(new Uint8Array([0, 0, 0]));
});


test("decrypt refuses to replace the encrypted original", async () => {
    destination = inputPath;
    const reply = await native.chooseDecryptAndSave(event, "transient canary");
    expect(reply.status).toBe("error");
    expect(new Uint8Array(await readFile(inputPath))).toEqual(new Uint8Array([1, 2, 3]));
});

test("decrypt replaces an existing permissive destination with a private file", async () => {
    if (process.platform === "win32") return;
    await writeFile(savePath, new Uint8Array([7]));
    await chmod(savePath, 0o644);
    expect(await native.chooseDecryptAndSave(event, "transient canary")).toEqual({ status: "ok" });
    expect((await stat(savePath)).mode & 0o777).toBe(0o600);
    expect(new Uint8Array(await readFile(inputPath))).toEqual(new Uint8Array([1, 2, 3]));
});
