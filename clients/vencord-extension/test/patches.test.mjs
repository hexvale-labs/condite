import assert from "node:assert/strict";
import { test } from "node:test";
import { build } from "../.vencord/node_modules/esbuild/lib/main.js";

const built = await build({
    stdin: { contents: 'export * from "./src/patches"; export * from "./src/routing"; export {canonicalizeMatch,canonicalizeReplace} from "./.vencord/src/utils/patches";', resolveDir: new URL("..", import.meta.url).pathname },
    bundle: true, write: false, format: "esm", platform: "node"
});
const { uploadPatch, navigationPatch, canonicalizeMatch, canonicalizeReplace, UploadRouter } = await import(`data:text/javascript;base64,${Buffer.from(built.outputFiles[0].text).toString("base64")}`);

test("actual Vencord patch normalization routes once and invokes the original method with its receiver", async () => {
    for (const name of ["e", "$files", "uploads_1"]) {
        const calls = [];
        const router = new UploadRouter({ mode: () => "automatic", limit: () => 10, choose: () => assert.fail("chooser"), upload: () => assert.fail("Condite"), append: () => assert.fail("draft"), report: () => assert.fail("error") });
        const plugin = { isDiscordBatch: files => router.isDiscordBatch(files), routeUploads: (files, native) => router.route(files, native) };
        const original = `({tag:"receiver",async uploadFiles(${name}){calls.push([this.tag,${name}]);}})`;
        const patched = original.replace(canonicalizeMatch(uploadPatch.replacement.match), canonicalizeReplace(uploadPatch.replacement.replace, "plugin"));
        assert.notEqual(original, patched);
        const instance = new Function("plugin", "calls", `return ${patched}`)(plugin, calls);
        const files = [{ filename: "small", channelId: "origin", status: "NOT_STARTED", item: { file: { size: 5 } }, once() {}, removeListener() {} }];
        await instance.uploadFiles(files);
        assert.equal(calls.length, 1);
        assert.equal(calls[0][0], "receiver");
        assert.deepEqual(calls[0][1], files);
        router.stop();
    }
});

test("sidebar patch inserts a React sibling after Friends", () => {
    const source = '(0,a.jsxs)("nav",{children:[(0,a.jsx)(b,{to:c.Routes.FRIENDS},"friends"),(0,a.jsx)(b,{to:c.Routes.APPLICATION_STORE},"store")]})';
    const patched = source.replace(canonicalizeMatch(navigationPatch.replacement.match), canonicalizeReplace(navigationPatch.replacement.replace, "plugin"));
    const a = { jsx: (type, props, key) => ({ type, props, key }), jsxs: (type, props) => props.children };
    const tree = new Function("a", "b", "c", "plugin", `return ${patched}`)(a, "link", { Routes: { FRIENDS: "/channels/@me", APPLICATION_STORE: "/store" } }, { renderUploadsNavigation: () => "uploads" });
    assert.equal(tree[0].key, "friends"); assert.equal(tree[1], "uploads"); assert.equal(tree[2].key, "store");
});
