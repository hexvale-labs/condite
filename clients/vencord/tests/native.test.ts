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
const previousApiKey = process.env.CONDITE_API_KEY;
const previousApiUrl = process.env.CONDITE_API_URL;
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
    constructor(base,key) { globalThis.__conditeNativeTest.client = {base,key}; }
    async health() { return {healthy:true}; }
    async listUploads() { return {uploads:[{id:'id',size_bytes:18446744073709551615n}],next_after:null}; }
    async confirm() { return {upload:{id:'id',size_bytes:18446744073709551615n},new_storage_size_bytes:18446744073709551615n}; }
    async updateRetention(id,request) { return {id,size_bytes:18446744073709551615n,expires_at:request.expires_at}; }
    async listShares() { return {shares:[],next_after:null}; }
    async download() { return {uri:'https://storage.example/private',expires_at:'later'}; }
    async revokeShare(id,share) { globalThis.__conditeNativeTest.trace.push('revoke:'+share); }
    async delete(id) { globalThis.__conditeNativeTest.trace.push('delete:'+id); }
    async upload(request,data) {
      globalThis.__conditeNativeTest.trace.push('upload');
      if (globalThis.__conditeNativeTest.uploadError) throw globalThis.__conditeNativeTest.uploadError;
      return {request_id:'id',confirmation:{upload:{id:'id'}}};
    }
    async createShare() { globalThis.__conditeNativeTest.trace.push('share'); return {url:'https://storage.example/share',expires_at:'later'};
    }
  }
};`);
const native = await import("../condite.desktop/native");
const event = {} as Parameters<typeof native.chooseAndUpload>[0];

beforeEach(async () => {
    process.env.CONDITE_API_KEY = "test-key";
    process.env.CONDITE_API_URL = "https://condite.example";
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
    for (const [name, value] of [["CONDITE_API_KEY", previousApiKey], ["CONDITE_API_URL", previousApiUrl], ["CONDITE_SDK_NODE_PATH", previousSdk], ["CONDITE_FFMPEG_PATH", previousFfmpeg], ["CONDITE_FFPROBE_PATH", previousFfprobe]]) {
        if (value === undefined) delete process.env[name!]; else process.env[name!] = value;
    }
    delete (globalThis as any).__conditeNativeTest;
    await rm(folder, { recursive: true, force: true });
});

test("native picker uploads prepared file and clears transient passphrase reference", async () => {
    const options = { stripMetadata: false, anonymizeFilename: true, passphrase: "transient canary" };
    const reply = await native.chooseAndUpload(event, "https://condite.example", options);
    expect(reply.status).toBe("ok");
    expect(trace).toEqual(["prepare", "upload", "share"]);
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

test("credentials cannot be sent to a different composer destination", async () => {
    const reply = await native.chooseAndUpload(event, "https://other.example", { stripMetadata: false, anonymizeFilename: false });
    expect(reply.status).toBe("error");
    expect(pickerCalls).toBe(0);
    expect(trace).toEqual([]);
    expect(JSON.stringify(reply)).not.toContain("test-key");
});


test("settings key authenticates every management operation with lossless IPC sizes", async () => {
    const url = "https://settings.example";
    const key = "settings-key";
    const call = (operation: Parameters<typeof native.manage>[3], id = "id", request: unknown = {}) => native.manage(event, url, key, operation, id, request);
    expect((await call("health")).status).toBe("ok");
    expect((globalThis as any).__conditeNativeTest.client).toEqual({base:"https://settings.example",key});
    const page = await call("listUploads");
    expect(page.status === "ok" && page.value.uploads[0].size_bytes).toBe("18446744073709551615");
    const confirmed = await call("confirm");
    expect(confirmed.status === "ok" && confirmed.value.new_storage_size_bytes).toBe("18446744073709551615");
    const retained = await call("updateRetention", "id", {expires_at:null});
    expect(retained.status === "ok" && retained.value.expires_at).toBeNull();
    expect((await call("createShare", "id", {expires_at:null,follows_upload:true})).status).toBe("ok");
    expect((await call("listShares")).status).toBe("ok");
    expect((await call("download")).status).toBe("ok");
    expect((await call("revokeShare", "id", "link")).status).toBe("ok");
    expect((await call("delete")).status).toBe("ok");
    expect(trace).toEqual(["share", "revoke:link", "delete:id"]);
    const invalid = await native.manage(event,url,key,"constructor" as any);
    expect(invalid.status).toBe("error");
});

test("upload recovery identifiers cross IPC without leaking the underlying exception", async () => {
    (globalThis as any).__conditeNativeTest.uploadError = Object.assign(new Error("PRIVATE_CANARY"), {request_id:"pending-id",stage:"put"});
    const reply = await native.chooseAndUpload(event, "https://condite.example", {stripMetadata:false,anonymizeFilename:false});
    expect(reply.status).toBe("error");
    if (reply.status === "error") {
        expect(reply.request_id).toBe("pending-id");
        expect(reply.stage).toBe("put");
        expect(reply.message).not.toContain("PRIVATE_CANARY");
        expect(reply.raw).toContain("PRIVATE_CANARY");
    }
});

test("HTTP upload failures retain readable problem details across IPC", async () => {
    const body = '{"title":"Sharing unavailable","detail":"Configure storage","debug":"raw-canary"}';
    (globalThis as any).__conditeNativeTest.uploadError = Object.assign(new Error(`HTTP 400: ${body}`), {status:400,body,request_id:"pending-id",stage:"confirm"});
    const reply = await native.chooseAndUpload(event, "https://condite.example", {stripMetadata:false,anonymizeFilename:false});
    expect(reply.status).toBe("error");
    if (reply.status === "error") {
        expect(reply.message).toContain("Sharing unavailable: Configure storage");
        expect(reply.raw).toContain("raw-canary");
        expect(reply.request_id).toBe("pending-id");
    }
});
