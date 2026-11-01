import assert from "node:assert/strict";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { test } from "node:test";
import { build } from "../.vencord/node_modules/esbuild/lib/main.js";

const output = new URL("../.vencord/dist/condite-native-test.cjs", import.meta.url).pathname;
await build({ entryPoints: [new URL("../src/native.ts", import.meta.url).pathname], outfile: output, bundle: true, format: "cjs", platform: "node" });
const native = createRequire(import.meta.url)(output);

test("packaged Electron SDK bridge exercises every SDK method and preserves u64 precision", async () => {
    const requests = [];
    const server = createServer(async (req, res) => {
        const chunks = [];
        for await (const chunk of req) chunks.push(chunk);
        const bytes = Buffer.concat(chunks);
        const url = new URL(req.url, "http://fixture");
        requests.push(`${req.method} ${url.pathname}`);
        const json = value => { res.setHeader("content-type", "application/json"); res.end(typeof value === "string" ? value : JSON.stringify(value)); };
        if (url.pathname === "/api/v1/health") return json({ healthy: true, started_at: "2026-10-31T00:00:00Z", runtime: "PT1S" });
        if (req.method === "GET" && url.pathname === "/api/v1/uploads") {
            assert.equal(url.searchParams.get("after"), "cursor&value");
            return json('{"uploads":[{"id":"id","filename":"a.txt","mime_type":"text/plain","size_bytes":18446744073709551615,"status":"Confirmed","created_at":"2026-10-31T00:00:00Z"}],"next_after":null}');
        }
        if (req.method === "POST" && url.pathname === "/api/v1/uploads") {
            const body = JSON.parse(bytes);
            assert.equal(body.size_bytes, 3);
            return json({ request_id: body.filename === "fail.txt" ? "fail" : "id", uri: `http://${req.headers.host}/storage`, headers: { "content-type": "text/plain", "content-length": "3" }, expires_at_utc: "2026-11-01T00:00:00Z" });
        }
        if (req.method === "PUT" && url.pathname === "/storage") {
            assert.equal(bytes.toString(), "abc"); assert.equal(req.headers.authorization, undefined); res.writeHead(200).end(); return;
        }
        if (url.pathname === "/api/v1/uploads/id/confirm") return json('{"public_uri":"https://storage.test/file","expires_at":"2026-11-01T00:00:00Z","new_storage_size_bytes":18446744073709551615}');
        if (url.pathname === "/api/v1/uploads/fail/confirm") { res.writeHead(409).end("not ready"); return; }
        if (req.method === "DELETE") { res.writeHead(204).end(); return; }
        res.writeHead(404).end();
    });
    await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
    const base = `http://127.0.0.1:${server.address().port}/api`;
    const value = async promise => { const response = await promise; assert.equal(response.ok, true, JSON.stringify(response)); return response.value; };
    try {
        assert.equal((await value(native.health(null, base))).healthy, true);
        const page = await value(native.listUploads(null, base, { limit: 2, after: "cursor&value" }));
        assert.equal(page.uploads[0].size_bytes, "18446744073709551615");
        const signed = await value(native.presign(null, base, "a.txt", "text/plain", "3"));
        await value(native.putUpload(null, base, signed, new Uint8Array(Buffer.from("abc"))));
        assert.equal((await value(native.confirm(null, base, signed.request_id))).new_storage_size_bytes, "18446744073709551615");
        const combined = await value(native.upload(null, base, "a.txt", "text/plain", new Uint8Array(Buffer.from("abc"))));
        assert.equal(combined.confirmation.public_uri, "https://storage.test/file");
        await value(native.deleteUpload(null, base, "id"));
        const failed = await native.upload(null, base, "fail.txt", "text/plain", new Uint8Array(Buffer.from("abc")));
        assert.equal(failed.ok, false); assert.equal(failed.error.status, 409); assert.equal(failed.error.stage, "confirm"); assert.equal(failed.error.request_id, "fail");
        const invalid = await native.health(null, "file:///tmp"); assert.equal(invalid.ok, false);
        assert.equal(requests.filter(value => value === "PUT /storage").length, 3);
    } finally {
        await new Promise(resolve => server.close(resolve));
    }
});
