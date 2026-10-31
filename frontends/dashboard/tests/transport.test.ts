import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { proxyRequest } from "../src/lib/proxy";
import { formatBytes, safeDownloadUrl } from "../src/lib/format";

test("proxy preserves u64 JSON and the configured API prefix", async () => {
  const server = createServer((req, res) => {
    assert.equal(req.url, "/prefix/v1/uploads?limit=2&after=cursor");
    assert.equal(req.headers.cookie, undefined);
    res.setHeader("Content-Type", "application/json");
    res.end('{"size_bytes":18446744073709551615}');
  });
  await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
  try {
    const address = server.address() as { port: number };
    const response = await proxyRequest(
      new Request("http://dashboard/api/v1/uploads?limit=2&after=cursor", {
        headers: { cookie: "private=value" },
      }),
      `http://127.0.0.1:${address.port}/prefix`,
    );
    assert.equal(await response.text(), '{"size_bytes":18446744073709551615}');
    assert.equal(response.headers.get("cache-control"), "no-store");
  } finally {
    server.closeAllConnections();
    server.close();
  }
});

test("proxy rejects unsupported routes and cross-origin mutations before forwarding", async () => {
  for (const path of [
    "/api/other",
    "/api/v1/uploads/id/extra",
    "/api/v1/uploads//confirm",
  ]) {
    assert.equal(
      (
        await proxyRequest(
          new Request(`http://dashboard${path}`),
          "http://127.0.0.1:1",
        )
      ).status,
      404,
    );
  }
  const request = new Request("http://dashboard/api/v1/uploads/id", {
    method: "DELETE",
    headers: { origin: "https://other.example" },
  });
  assert.equal((await proxyRequest(request, "http://127.0.0.1:1")).status, 403);
});

test("formats the complete u64 range and refuses unsafe download links", () => {
  assert.equal(formatBytes(18446744073709551615n, "en"), "16 EiB");
  assert.equal(formatBytes(1024n, "en"), "1 KiB");
  assert.equal(safeDownloadUrl("javascript:alert(1)"), null);
  assert.equal(safeDownloadUrl("https://user:password@example.com/f"), null);
  assert.equal(
    safeDownloadUrl("https://bucket.example/f?signature=x"),
    "https://bucket.example/f?signature=x",
  );
});

test('proxy stops reading an oversized streaming request', async () => {
  let cancelled = false;
  let chunks = 0;
  const body = new ReadableStream<Uint8Array>({
    pull(controller) {
      if (chunks++ < 3) controller.enqueue(new Uint8Array(20_000));
      else controller.close();
    },
    cancel() { cancelled = true; },
  });
  const request = new Request('http://dashboard/api/v1/uploads', {
    method: 'POST', headers: { origin: 'http://dashboard', 'content-type': 'application/json' },
    body,
    ...{ duplex: 'half' },
  });
  const response = await proxyRequest(request, 'http://127.0.0.1:1');
  assert.equal(response.status, 413);
  assert.equal(cancelled, true);
});
