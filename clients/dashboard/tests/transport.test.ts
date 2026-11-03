import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { proxyRequest as authenticatedProxy } from "../src/lib/proxy";
import { formatBytes, safeDownloadUrl } from "../src/lib/format";

const proxyRequest = (request: Request, base: string, key = "test-session") => authenticatedProxy(request, base, key);
test("proxy requires a session", async () => {
  assert.equal((await authenticatedProxy(new Request("http://dashboard/api/v1/account"), "http://127.0.0.1:1")).status, 401);
});

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

test("proxy stops reading an oversized streaming request", async () => {
  let cancelled = false;
  let chunks = 0;
  const body = new ReadableStream<Uint8Array>({
    pull(controller) {
      if (chunks++ < 3) controller.enqueue(new Uint8Array(20_000));
      else controller.close();
    },
    cancel() {
      cancelled = true;
    },
  });
  const request = new Request("http://dashboard/api/v1/uploads", {
    method: "POST",
    headers: { origin: "http://dashboard", "content-type": "application/json" },
    body,
    ...{ duplex: "half" },
  });
  const response = await proxyRequest(request, "http://127.0.0.1:1");
  assert.equal(response.status, 413);
  assert.equal(cancelled, true);
});

test("proxy uses only the configured server-side API key", async () => {
  let authorization: string | undefined;
  const server = createServer((req, res) => {
    authorization = req.headers.authorization;
    res.end("{}");
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    const address = server.address() as { port: number };
    const response = await proxyRequest(
      new Request("http://dashboard/api/v1/uploads", {
        headers: { authorization: "Bearer attacker" },
      }),
      `http://127.0.0.1:${address.port}`,
      "server-secret",
    );
    assert.equal(response.status, 200);
    assert.equal(authorization, "Bearer server-secret");
    assert.equal((await response.text()).includes("server-secret"), false);
  } finally {
    server.closeAllConnections();
    server.close();
  }
});

test("sharing, owner downloads and PATCH retention preserve raw bodies and reject foreign origins", async () => {
  const calls: string[] = [];
  const raw = '{"expires_at":null,"size_bytes":18446744073709551615}';
  const server = createServer(async (req, res) => {
    let body = "";
    for await (const chunk of req) body += chunk;
    calls.push(`${req.method} ${req.url} ${body}`);
    res.end("{}");
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const base = `http://127.0.0.1:${(server.address() as { port: number }).port}`;
  try {
    for (const [method, path] of [
      ["GET", "id/shares"],
      ["POST", "id/shares"],
      ["DELETE", "id/shares/share-id"],
      ["PATCH", "id/retention"],
      ["POST", "id/download"],
    ]) {
      const body = method === "PATCH" || method === "POST" ? raw : undefined;
      const response = await proxyRequest(
        new Request(`http://dashboard/api/v1/uploads/${path}`, {
          method,
          body,
          headers: {
            origin: "http://dashboard",
            "content-type": "application/json",
          },
        }),
        base,
      );
      assert.equal(response.status, 200, `${method} ${path}`);
      if (body) assert.ok(calls.at(-1)?.endsWith(raw));
      if (method !== "GET") {
        assert.equal(
          (
            await proxyRequest(
              new Request(`http://dashboard/api/v1/uploads/${path}`, {
                method,
                body,
                headers: {
                  origin: "https://foreign.example",
                  "content-type": "application/json",
                },
              }),
              base,
            )
          ).status,
          403,
        );
      }
    }
  } finally {
    server.closeAllConnections();
    server.close();
  }
});

test("bodyless SDK mutations accept an empty request stream without Content-Type", async () => {
  const server = createServer(async (req, res) => {
    let body = "";
    for await (const chunk of req) body += chunk;
    assert.equal(body, "");
    res.end("{}");
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const base = `http://127.0.0.1:${(server.address() as { port: number }).port}`;
  try {
    for (const action of ["confirm", "download"]) {
      const response = await proxyRequest(
        new Request(`http://dashboard/api/v1/uploads/id/${action}`, {
          method: "POST",
          headers: { origin: "http://dashboard" },
          body: new Uint8Array(0),
        }),
        base,
      );
      assert.equal(response.status, 200);
    }
    const rejected = await proxyRequest(
      new Request("http://dashboard/api/v1/uploads/id/retention", {
        method: "PATCH",
        headers: { origin: "http://dashboard" },
        body: new Uint8Array([1]),
      }),
      base,
    );
    assert.equal(rejected.status, 415);
  } finally {
    server.closeAllConnections();
    server.close();
  }
});

test("proxy streams content and range headers using the session credential",async()=>{
  const server=createServer((req,res)=>{
    assert.equal(req.url,"/v1/uploads/id/content");
    assert.equal(req.headers.range,"bytes=1-3");
    assert.equal(req.headers.authorization,"Bearer session-token");
    res.writeHead(206,{"content-type":"application/octet-stream","content-range":"bytes 1-3/10","content-length":"3","accept-ranges":"bytes"});
    res.end("abc");
  });
  await new Promise<void>(resolve=>server.listen(0,"127.0.0.1",resolve));
  try{
    const {port}=server.address() as {port:number};
    const response=await proxyRequest(new Request("http://dashboard/api/v1/uploads/id/content",{headers:{Range:"bytes=1-3"}}),`http://127.0.0.1:${port}`,"session-token");
    assert.equal(response.status,206);assert.equal(response.headers.get("content-range"),"bytes 1-3/10");assert.equal(await response.text(),"abc");
  }finally{server.closeAllConnections();server.close();}
});

test("a progressing stream can outlive the initial response deadline",async()=>{
  const server=createServer((_req,res)=>{
    res.writeHead(200,{"content-type":"application/octet-stream"});res.flushHeaders();
    let sent=0;
    const interval=setInterval(()=>{res.write("x");if(++sent===10){clearInterval(interval);res.end();}},40);
    res.on("close",()=>clearInterval(interval));
  });
  await new Promise<void>(resolve=>server.listen(0,"127.0.0.1",resolve));
  try{
    const {port}=server.address() as {port:number};
    const response=await authenticatedProxy(new Request("http://dashboard/api/v1/uploads/id/content"),`http://127.0.0.1:${port}`,"session",150);
    assert.equal(response.status,200);
    assert.equal(await response.text(),"xxxxxxxxxx");
  }finally{server.closeAllConnections();server.close();}
});

test("a stalled upstream body is aborted after an idle deadline",async()=>{
  const server=createServer((_req,res)=>{res.writeHead(200,{"content-type":"application/octet-stream"});res.write("first");});
  await new Promise<void>(resolve=>server.listen(0,"127.0.0.1",resolve));
  try{
    const {port}=server.address() as {port:number};
    const response=await authenticatedProxy(new Request("http://dashboard/api/v1/uploads/id/content"),`http://127.0.0.1:${port}`,"session",150);
    assert.equal(response.status,200);
    await assert.rejects(response.text());
  }finally{server.closeAllConnections();server.close();}
});

test("cancelling a proxied download closes its upstream response",async()=>{
  let resolveClosed!:()=>void;
  const closed=new Promise<void>(resolve=>{resolveClosed=resolve;});
  const server=createServer((_req,res)=>{res.writeHead(200,{"content-type":"application/octet-stream"});res.write("first");res.on("close",resolveClosed);});
  await new Promise<void>(resolve=>server.listen(0,"127.0.0.1",resolve));
  let timer:ReturnType<typeof setTimeout>|undefined;
  try{
    const {port}=server.address() as {port:number};
    const response=await authenticatedProxy(new Request("http://dashboard/api/v1/uploads/id/content"),`http://127.0.0.1:${port}`,"session",1500);
    const reader=response.body!.getReader();
    assert.equal(new TextDecoder().decode((await reader.read()).value),"first");
    await reader.cancel();
    await Promise.race([closed,new Promise<never>((_,reject)=>{timer=setTimeout(()=>reject(new Error("upstream did not close")),1500);})]);
  }finally{clearTimeout(timer);server.closeAllConnections();server.close();}
});
