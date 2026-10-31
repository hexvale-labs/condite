const allowed = (path: string, method: string) =>
  (path === "v1/health" && method === "GET") ||
  (path === "v1/uploads" && ["GET", "POST"].includes(method)) ||
  (/^v1\/uploads\/[a-zA-Z0-9-]+$/.test(path) && method === "DELETE") ||
  (/^v1\/uploads\/[a-zA-Z0-9-]+\/confirm$/.test(path) && method === "POST");

export async function proxyRequest(
  request: Request,
  base: string,
): Promise<Response> {
  const url = new URL(request.url);
  const path = url.pathname.replace(/^\/api\//, "");
  const reply = (status: number) =>
    new Response(null, { status, headers: { "Cache-Control": "no-store" } });
  if (!allowed(path, request.method)) return reply(404);
  if (request.method !== "GET" && request.headers.get("origin") !== url.origin)
    return reply(403);
  if (
    request.method === "POST" &&
    path === "v1/uploads" &&
    !request.headers.get("content-type")?.startsWith("application/json")
  )
    return reply(415);
  const length = Number(request.headers.get("content-length"));
  if (length > 16_384) return reply(413);
  const upstream = new URL(`${base.replace(/\/$/, "")}/${path}`);
  if (
    !["http:", "https:"].includes(upstream.protocol) ||
    upstream.username ||
    upstream.password ||
    upstream.hash
  )
    return reply(502);
  upstream.search = url.search;
  try {
    // Never parse JSON here: Rust's u64 fields exceed JavaScript's Number precision.
    let body: Uint8Array<ArrayBuffer> | undefined;
    if (request.method === "POST" && request.body) {
      const reader = request.body.getReader();
      const chunks: Uint8Array[] = [];
      let size = 0;
      try {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          size += value.byteLength;
          if (size > 16_384) {
            await reader.cancel();
            return reply(413);
          }
          chunks.push(value);
        }
      } finally {
        reader.releaseLock();
      }
      body = new Uint8Array(size);
      let offset = 0;
      for (const chunk of chunks) {
        body.set(chunk, offset);
        offset += chunk.byteLength;
      }
    }
    const response = await fetch(upstream, {
      method: request.method,
      body: body || undefined,
      headers: body ? { "Content-Type": "application/json" } : {},
      redirect: "error",
      signal: AbortSignal.timeout(15_000),
    });
    return new Response(response.body, {
      status: response.status,
      headers: {
        "Content-Type":
          response.headers.get("content-type") || "application/json",
        "Cache-Control": "no-store",
      },
    });
  } catch {
    return reply(502);
  }
}
