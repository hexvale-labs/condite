// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import http from "node:http";
import { createRequire } from "node:module";

import { uploadPrepared, type Sdk } from "../condite.desktop/workflow";

const sdkPath = process.env.CONDITE_REAL_SDK_PATH;
test.skipIf(!sdkPath)("generated native SDK cleans, anonymizes, encrypts and uploads through the plugin workflow", async () => {
    const sdk = createRequire(import.meta.url)(sdkPath!) as Sdk;
    const fixtures = JSON.parse(await readFile(new URL("../../../scripts/sdk/fixtures/privacy.json", import.meta.url), "utf8"));
    const input = new Uint8Array(Buffer.from(fixtures["image.png"], "base64"));
    const calls: string[] = [];
    let presign: { filename: string; mime_type: string; size_bytes: number; } | undefined;
    let stored = new Uint8Array();
    const server = http.createServer(async (request, response) => {
        calls.push(`${request.method} ${request.url}`);
        const chunks: Buffer[] = [];
        for await (const chunk of request) chunks.push(Buffer.from(chunk));
        const body = Buffer.concat(chunks);
        response.setHeader("Content-Type", "application/json");
        if (request.url === "/v1/uploads") {
            presign = JSON.parse(body.toString());
            response.end(JSON.stringify({ request_id: "id", uri: `http://${request.headers.host}/storage`, headers: {}, expires_at_utc: "later" }));
        } else if (request.url === "/storage") {
            stored = new Uint8Array(body);
            response.writeHead(200).end();
        } else if (request.url === "/v1/uploads/id/confirm") {
            response.end(JSON.stringify({ upload: {id:"id",filename:presign!.filename,mime_type:presign!.mime_type,size_bytes:stored.byteLength,status:"Confirmed",created_at:"2026-11-01T00:00:00Z",expires_at:null,visibility:"Private",deletion_pending:false}, new_storage_size_bytes: stored.byteLength }));
        } else if (request.url === "/v1/uploads/id/shares") {
            expect(JSON.parse(body.toString())).toEqual({follows_upload:true,expires_at:null});
            response.end(JSON.stringify({id:"share",upload_id:"id",url:"https://storage.example/direct",created_at:"2026-11-01T00:00:00Z",expires_at:null,follows_upload:true,status:"Active"}));
        } else response.writeHead(404).end();
    });
    await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
    try {
        const address = server.address() as { port: number; };
        const baseUrl = `http://127.0.0.1:${address.port}`;
        const result = await uploadPrepared(sdk, baseUrl, input, "PRIVATE_CANARY.png", {
            stripMetadata: true, anonymizeFilename: true, passphrase: "synthetic local test passphrase"
        });
        expect(calls).toEqual(["POST /v1/uploads", "PUT /storage", "POST /v1/uploads/id/confirm", "POST /v1/uploads/id/shares"]);
        expect(result.filename).not.toContain("PRIVATE_CANARY");
        expect(result.filename).toEndWith(".age");
        expect(result.report.metadataStripped).toBe(true);
        expect(result.report.filenameAnonymized).toBe(true);
        expect(result.report.encrypted).toBe(true);
        expect(presign!.mime_type).toBe("application/octet-stream");
        expect(BigInt(presign!.size_bytes)).toBe(BigInt(stored.byteLength));
        const plain = await new sdk.MetadataSanitizer({ maxInputBytes: 300n * 1024n * 1024n, maxOutputBytes: 100n * 1024n * 1024n, maxInflatedBytes: 512n * 1024n * 1024n, maxEntryBytes: 64n * 1024n * 1024n, maxEntries: 10000 }).decrypt(stored, "synthetic local test passphrase");
        expect(new TextDecoder().decode(plain)).not.toContain("PRIVATE_CANARY");
        expect(Array.from(plain.subarray(0, 8))).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
        await expect(uploadPrepared(sdk, baseUrl, new Uint8Array([1, 2, 3]), "unsupported.bin", { stripMetadata: true, anonymizeFilename: false })).rejects.toThrow("Upload was not started");
        expect(calls).toHaveLength(4);
    } finally {
        server.closeAllConnections();
        await new Promise<void>(resolve => server.close(() => resolve()));
    }
}, 60_000);
