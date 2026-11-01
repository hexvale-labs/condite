import assert from "node:assert/strict";
import { test } from "node:test";
import { build } from "../.vencord/node_modules/esbuild/lib/main.js";

globalThis.conditeSettings = { store: { apiBase: "http://origin.test" } };
const built = await build({
    entryPoints: [new URL("../.vencord/src/userplugins/condite/sdk.ts", import.meta.url).pathname], bundle: true, write: false, format: "esm", platform: "node",
    plugins: [{ name: "settings-fixture", setup(build) {
        build.onResolve({ filter: /^\.\/settings$/ }, () => ({ path: "settings", namespace: "fixture" }));
        build.onResolve({ filter: /^@webpack\/common$/ }, () => ({ path: "common", namespace: "fixture" }));
        build.onLoad({ filter: /.*/, namespace: "fixture" }, ({ path }) => ({ contents: path === "settings"
            ? "export const settings = globalThis.conditeSettings;"
            : 'export const LocaleStore = { locale: "pl" }; export const useStateFromStores = (_, selector) => selector();' }));
    } }]
});
const { createClient, uploadFile } = await import(`data:text/javascript;base64,${Buffer.from(built.outputFiles[0].text).toString("base64")}`);
const confirmation = { public_uri: "https://storage.test/file", expires_at: "2026-11-01T00:00:00Z", new_storage_size_bytes: "18446744073709551615" };
const ok = value => Promise.resolve({ ok: true, value });

test("renderer adapter restores bigint for every u64 response", async () => {
    globalThis.VencordNative = { pluginHelpers: { Condite: {
        confirm: () => ok(confirmation),
        upload: () => ok({ request_id: "id", confirmation }),
        listUploads: () => ok({ uploads: [{ size_bytes: "18446744073709551615" }], next_after: null })
    } } };
    const sdk = createClient();
    assert.equal((await sdk.confirm("id")).new_storage_size_bytes, 18446744073709551615n);
    assert.equal((await sdk.upload({ name: "a", type: "text/plain" }, new Uint8Array())).confirmation.new_storage_size_bytes, 18446744073709551615n);
    assert.equal((await sdk.listUploads()).uploads[0].size_bytes, 18446744073709551615n);
});

test("staged upload snapshots its endpoint and exposes failed stage/request ID", async () => {
    const bases = [];
    globalThis.conditeSettings.store.apiBase = "http://origin.test";
    globalThis.VencordNative = { pluginHelpers: { Condite: {
        presign: base => { bases.push(base); globalThis.conditeSettings.store.apiBase = "http://changed.test"; return ok({ request_id: "pending", uri: "https://storage.test/put", headers: {} }); },
        putUpload: base => { bases.push(base); return ok(undefined); },
        confirm: base => { bases.push(base); return Promise.resolve({ ok: false, error: { message: "not ready", status: 409 } }); }
    } } };
    await assert.rejects(uploadFile(new File(["abc"], "a.txt")), error => error.request_id === "pending" && error.stage === "confirm" && error.status === 409);
    assert.deepEqual(bases, ["http://origin.test", "http://origin.test", "http://origin.test"]);
});

test("oversized upload errors use Discord's language before making any request", async () => {
    await assert.rejects(uploadFile({ size: 101 * 1024 * 1024 }), /Do 100 MiB na plik/);
});
