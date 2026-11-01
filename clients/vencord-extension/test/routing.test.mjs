import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { test } from "node:test";
import { build } from "../.vencord/node_modules/esbuild/lib/main.js";

const result = await build({ entryPoints: [new URL("../src/routing.ts", import.meta.url).pathname], bundle: true, write: false, format: "esm", platform: "node" });
const { UploadRouter, formatLinks } = await import(`data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString("base64")}`);

function file(name, size = 5, channelId = "origin") {
    return Object.assign(new EventEmitter(), {
        filename: name, channelId, status: "NOT_STARTED", removed: false,
        item: { file: { name, size } },
        removeFromMsgDraft() { this.removed = true; }
    });
}

function harness(overrides = {}) {
    const links = [], uploaded = [], errors = [], choices = [];
    const router = new UploadRouter({
        mode: () => "condite", limit: () => 10,
        choose: async (files, failure) => { choices.push({ files, failure }); return files.map(() => "condite"); },
        upload: async upload => { uploaded.push(upload); return `https://storage.test/${uploaded.length}`; },
        append: (channel, text) => { links.push({ channel, text }); },
        report: (name, error) => errors.push({ name, error }),
        ...overrides
    });
    return { router, links, uploaded, errors, choices };
}

test("multiple files preserve order and duplicate names, with links only in the originating draft", async () => {
    const h = harness();
    const files = [file("same.png"), file("same.png"), file("other.txt")];
    let native = 0;
    await h.router.route(files, async () => { native++; });
    assert.equal(native, 0);
    assert.deepEqual(h.links, [{ channel: "origin", text: "\n\n[same.png](https://storage.test/1)\n[same.png (2)](https://storage.test/2)\n[other.txt](https://storage.test/3)" }]);
    assert.ok(files.every(f => f.removed));
});

test("automatic routing evaluates each file and preserves native uploads", async () => {
    const h = harness({ mode: () => "automatic" });
    const small = file("small", 10), large = file("large", 11);
    let native;
    await h.router.route([small, large], async files => { native = files; files.forEach(f => { f.status = "COMPLETED"; f.emit("complete"); }); });
    assert.deepEqual(native, [small]);
    assert.deepEqual(h.uploaded, [large]);
    assert.equal(small.removed, false);
    assert.equal(small.listenerCount("error"), 0);
});

test("cancelled chooser preserves attachments and does not upload", async () => {
    const h = harness({ mode: () => "choose", choose: async () => null });
    const f = file("keep");
    await h.router.route([f], () => assert.fail("native upload"));
    assert.equal(f.removed, false);
    assert.equal(h.uploaded.length, 0);
});

test("partial Condite failures retain failed attachments and insert successful links", async () => {
    const h = harness({ upload: async f => { if (f.filename === "bad") throw new Error("offline"); return "https://storage.test/ok"; } });
    const good = file("good"), bad = file("bad");
    await h.router.route([good, bad], () => assert.fail("native upload"));
    assert.equal(good.removed, true);
    assert.equal(bad.removed, false);
    assert.equal(h.errors.length, 1);
    assert.equal(h.links.length, 1);
});

test("native error event and rejected callback offer one retry, never duplicate completed files", async () => {
    const h = harness({ mode: () => "automatic" });
    const good = file("good"), bad = file("bad");
    await h.router.route([good, bad], async () => {
        good.status = "COMPLETED"; good.emit("complete");
        bad.status = "ERROR"; bad.emit("error", new Error("network"));
        throw new Error("batch failed");
    });
    await h.router.idle();
    assert.deepEqual(h.uploaded, [bad]);
    assert.equal(h.choices.length, 1);
    assert.equal(h.choices[0].failure, true);
    assert.equal(good.removed, false);
});

test("declining native recovery keeps the attachment", async () => {
    const h = harness({ mode: () => "automatic", choose: async () => null });
    const f = file("bad");
    await h.router.route([f], async () => { f.status = "ERROR"; throw new Error("offline"); });
    await h.router.idle();
    assert.equal(h.uploaded.length, 0);
    assert.equal(f.removed, false);
});

test("late native error events still offer recovery after the batch promise resolves", async () => {
    const h = harness({ mode: () => "automatic" });
    const f = file("late");
    await h.router.route([f], async () => {});
    f.status = "ERROR"; f.emit("error", new Error("late network error"));
    await h.router.idle();
    assert.deepEqual(h.uploaded, [f]);
});

test("native files that complete while the recovery prompt is open are not uploaded twice", async () => {
    const f = file("late success");
    const h = harness({ mode: () => "automatic", choose: async () => { f.status = "COMPLETED"; return ["condite"]; } });
    await h.router.route([f], async () => { f.status = "ERROR"; throw new Error("uncertain batch"); });
    await h.router.idle();
    assert.equal(h.uploaded.length, 0);
});

test("stopping removes native event observers", async () => {
    const h = harness({ mode: () => "automatic" });
    const f = file("pending");
    await h.router.route([f], async () => {});
    assert.equal(f.listenerCount("error"), 1);
    h.router.stop();
    assert.equal(f.listenerCount("error"), 0);
    assert.equal(f.listenerCount("complete"), 0);
});

test("failed draft insertion leaves successful files attached for recovery", async () => {
    const h = harness({ append: () => { throw new Error("draft unavailable"); } });
    const f = file("keep");
    await h.router.route([f], () => assert.fail("native upload"));
    assert.equal(f.removed, false);
    assert.equal(h.errors.length, 1);
});

test("links escape Markdown filenames and reject non-HTTP destinations", () => {
    assert.equal(formatLinks([{ name: "a[link]\n.png", url: "https://storage.test/a(b)" }]), "\n\n[a\\[link\\] .png](https://storage.test/a%28b%29)");
    assert.throws(() => formatLinks([{ name: "x", url: "javascript:alert(1)" }]));
});
