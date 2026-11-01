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
        if (url.pathname.startsWith("/api/")) assert.equal(req.headers.authorization, "Bearer fixture-key");
        const json = value => { res.setHeader("content-type", "application/json"); res.end(typeof value === "string" ? value : JSON.stringify(value)); };
        if (url.pathname === "/api/v1/health") return json({ healthy: true, started_at: "2026-10-31T00:00:00Z", runtime: "PT1S" });
        if (req.method === "GET" && url.pathname === "/api/v1/uploads") {
            assert.equal(url.searchParams.get("after"), "cursor&value");
            return json('{"uploads":[{"id":"id","filename":"a.txt","mime_type":"text/plain","size_bytes":18446744073709551615,"status":"Confirmed","created_at":"2026-10-31T00:00:00Z","expires_at":null,"visibility":"Private","deletion_pending":false}],"next_after":null}');
        }
        if (req.method === "POST" && url.pathname === "/api/v1/uploads") {
            const body = JSON.parse(bytes);
            assert.equal(body.size_bytes, 3);
            return json({ request_id: body.filename === "fail.txt" ? "fail" : "id", uri: `http://${req.headers.host}/storage`, headers: { "content-type": "text/plain", "content-length": "3" }, expires_at_utc: "2026-11-01T00:00:00Z" });
        }
        if (req.method === "PUT" && url.pathname === "/storage") {
            assert.equal(bytes.toString(), "abc"); assert.equal(req.headers.authorization, undefined); res.writeHead(200).end(); return;
        }
        const row = {id:"id",filename:"a.txt",mime_type:"text/plain",size_bytes:3,status:"Confirmed",created_at:"2026-11-01T00:00:00Z",expires_at:null,visibility:"Private",deletion_pending:false};
        const share = {id:"share",upload_id:"id",url:"https://storage.test/direct",created_at:row.created_at,expires_at:null,follows_upload:true,status:"Active"};
        if (url.pathname === "/api/v1/uploads/id/confirm") return json('{"upload":'+JSON.stringify(row)+',"new_storage_size_bytes":18446744073709551615}');
        if (url.pathname === "/api/v1/uploads/id/download") return json({uri:"https://storage.test/private?signed",expires_at:"2026-11-01T00:00:00Z"});
        if (url.pathname === "/api/v1/uploads/id/retention") { assert.equal(req.method,"PATCH"); assert.deepEqual(JSON.parse(bytes),{expires_at:null}); return json(row); }
        if (url.pathname === "/api/v1/uploads/id/shares") return json(req.method === "POST" ? share : {shares:[share],next_after:null});
        if (url.pathname === "/api/v1/uploads/fail/confirm") { res.writeHead(409).end("not ready"); return; }
        if (req.method === "DELETE") { res.writeHead(204).end(); return; }
        res.writeHead(404).end();
    });
    await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
    const base = `http://127.0.0.1:${server.address().port}/api`;
    const value = async promise => { const response = await promise; assert.equal(response.ok, true, JSON.stringify(response)); return response.value; };
    try {
        assert.equal((await value(native.health(null, base, "fixture-key"))).healthy, true);
        const page = await value(native.listUploads(null, base, { limit: 2, after: "cursor&value" }, "fixture-key"));
        assert.equal(page.uploads[0].size_bytes, "18446744073709551615");
        const signed = await value(native.presign(null, base, "a.txt", "text/plain", "3", "fixture-key", {expires_at:null}));
        await value(native.putUpload(null, base, signed, new Uint8Array(Buffer.from("abc")), "fixture-key"));
        assert.equal((await value(native.confirm(null, base, signed.request_id, "fixture-key"))).new_storage_size_bytes, "18446744073709551615");
        const combined = await value(native.upload(null, base, "a.txt", "text/plain", new Uint8Array(Buffer.from("abc")), "fixture-key", {expires_at:null}));
        assert.equal(combined.confirmation.upload.id, "id");
        assert.equal((await value(native.createShare(null,base,"id",{follows_upload:true,expires_at:null}, "fixture-key"))).url,"https://storage.test/direct");
        assert.equal((await value(native.listShares(null,base,"id",{}, "fixture-key"))).shares.length,1);
        await value(native.revokeShare(null,base,"id","share", "fixture-key"));
        assert.equal((await value(native.updateRetention(null,base,"id",{expires_at:null}, "fixture-key"))).expires_at,null);
        assert.equal((await value(native.download(null,base,"id", "fixture-key"))).uri,"https://storage.test/private?signed");
        await value(native.deleteUpload(null, base, "id", "fixture-key"));
        const failed = await native.upload(null, base, "fail.txt", "text/plain", new Uint8Array(Buffer.from("abc")), "fixture-key");
        assert.equal(failed.ok, false); assert.equal(failed.error.status, 409); assert.equal(failed.error.stage, "confirm"); assert.equal(failed.error.request_id, "fail");
        const invalid = await native.health(null, "file:///tmp"); assert.equal(invalid.ok, false);
        assert.equal(requests.filter(value => value === "PUT /storage").length, 3);
    } finally {
        await new Promise(resolve => server.close(resolve));
    }
});
