import { createServer } from "node:http";
const ids = [
  "00000000-0000-7000-8000-000000000001",
  "00000000-0000-7000-8000-000000000002",
];
const initial = () => [
  {
    id: ids[0]!,
    filename: "notes.txt",
    mime_type: "text/plain",
    size_bytes: 18446744073709551615n,
    status: "Confirmed",
    created_at: "2026-10-31T00:00:00Z",
  },
  {
    id: ids[1]!,
    filename: "pending.txt",
    mime_type: "text/plain",
    size_bytes: 3n,
    status: "Pending",
    created_at: "2026-10-31T01:00:00Z",
  },
];
let rows = initial();
let calls: string[] = [];
let failConfirm = false;
let failPut = false;
let failDelete = false;
let unavailable = false;
let stalled = false;
let nextId = 3;
const json = (value: unknown) =>
  JSON.stringify(value, (_, v) =>
    typeof v === "bigint" ? `__u64:${v}` : v,
  ).replace(/"__u64:(\d+)"/g, "$1");
const server = createServer(async (req, res) => {
  const url = new URL(req.url!, "http://127.0.0.1:4322");
  res.setHeader("Access-Control-Allow-Origin", "http://127.0.0.1:4321");
  res.setHeader(
    "Access-Control-Allow-Methods",
    "GET, POST, DELETE, PUT, OPTIONS",
  );
  res.setHeader("Access-Control-Allow-Headers", "Content-Type");
  if (req.method === "OPTIONS") {
    res.end();
    return;
  }
  let body = "";
  for await (const chunk of req) body += chunk;
  const send = (status: number, value: unknown) => {
    res.writeHead(status, { "Content-Type": "application/json" });
    res.end(json(value));
  };
  if (url.pathname === "/__reset") {
    rows = initial();
    calls = [];
    failConfirm = false;
    failPut = false;
    failDelete = false;
    unavailable = false;
    stalled = false;
    return send(200, {});
  }
  if (url.pathname === "/__calls") return send(200, calls);
  if (url.pathname === "/__failure") {
    const value = JSON.parse(body);
    failConfirm = value.confirm ?? false;
    failPut = value.put ?? false;
    failDelete = value.delete ?? false;
    unavailable = value.unavailable ?? false;
    stalled = value.stalled ?? false;
    return send(200, {});
  }
  calls.push(`${req.method} ${url.pathname}`);
  if (stalled && url.pathname.startsWith("/v1/")) return;
  if (unavailable && url.pathname.startsWith("/v1/"))
    return send(503, { error: "fixture offline" });
  if (url.pathname === "/v1/health")
    return send(200, {
      healthy: true,
      started_at: "2026-10-31T00:00:00Z",
      runtime: "fixture",
    });
  if (url.pathname === "/v1/uploads" && req.method === "GET") {
    const after = url.searchParams.get("after");
    const all = after ? rows.filter((r) => r.id > after) : rows;
    const page = all.slice(0, 2);
    return send(200, {
      uploads: page,
      next_after: all.length > 2 ? page.at(-1)!.id : null,
    });
  }
  if (url.pathname === "/v1/uploads" && req.method === "POST") {
    const value = JSON.parse(body);
    const id = `00000000-0000-7000-8000-${String(nextId++).padStart(12, "0")}`;
    rows.push({
      ...value,
      size_bytes: BigInt(value.size_bytes),
      id,
      status: "Pending",
      created_at: "2026-10-31T02:00:00Z",
    });
    return send(200, {
      request_id: id,
      uri: `http://127.0.0.1:4322/storage/${id}`,
      headers: {
        "content-type": value.mime_type,
        "content-length": String(value.size_bytes),
      },
      expires_at_utc: "2099-01-01T00:00:00Z",
    });
  }
  if (url.pathname.startsWith("/storage/") && req.method === "PUT")
    return send(failPut ? 503 : 200, {});
  const confirm = url.pathname.match(/^\/v1\/uploads\/([^/]+)\/confirm$/);
  if (confirm && req.method === "POST") {
    const row = rows.find((r) => r.id === confirm[1]);
    if (!row) return send(404, {});
    if (failConfirm) return send(503, { error: "confirmation failed" });
    row.status = "Confirmed";
    return send(200, {
      public_uri: `http://127.0.0.1:4322/download/${row.id}`,
      expires_at: "2099-01-01T00:00:00Z",
      new_storage_size_bytes: 18446744073709551615n,
    });
  }
  const remove = url.pathname.match(/^\/v1\/uploads\/([^/]+)$/);
  if (remove && req.method === "DELETE") {
    if (failDelete) return send(503, { error: "deletion failed" });
    rows = rows.filter((r) => r.id !== remove[1]);
    res.writeHead(204);
    res.end();
    return;
  }
  if (url.pathname.startsWith("/download/")) {
    res.end("file");
    return;
  }
  send(404, {});
});
server.listen(4322, "127.0.0.1");
