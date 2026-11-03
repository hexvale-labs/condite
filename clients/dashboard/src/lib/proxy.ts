const allowed = (path: string, method: string) =>
  (path === "v1/account" && method === "GET") ||
  (path === "v1/control" && method === "POST") ||
  (/^v1\/uploads\/[a-zA-Z0-9-]+\/content$/.test(path) &&
    ["GET", "HEAD"].includes(method)) ||
  (path === "v1/health" && method === "GET") ||
  (path === "v1/uploads" && ["GET", "POST"].includes(method)) ||
  (/^v1\/uploads\/[a-zA-Z0-9-]+$/.test(path) && method === "DELETE") ||
  (/^v1\/uploads\/[a-zA-Z0-9-]+\/(confirm|download)$/.test(path) &&
    method === "POST") ||
  (/^v1\/uploads\/[a-zA-Z0-9-]+\/shares$/.test(path) &&
    ["GET", "POST"].includes(method)) ||
  (/^v1\/uploads\/[a-zA-Z0-9-]+\/shares\/[a-zA-Z0-9-]+$/.test(path) &&
    method === "DELETE") ||
  (/^v1\/uploads\/[a-zA-Z0-9-]+\/retention$/.test(path) && method === "PATCH");

export async function proxyRequest(
  request: Request,
  base: string,
  apiKey?: string,
  timeoutMs = 15_000,
): Promise<Response> {
  const url = new URL(request.url);
  const path = url.pathname.replace(/^\/api\//, "");
  const reply = (status: number) =>
    new Response(null, { status, headers: { "Cache-Control": "no-store" } });
  if (!apiKey) return reply(401);
  if (!allowed(path, request.method)) return reply(404);
  if (
    !["GET", "HEAD"].includes(request.method) &&
    request.headers.get("origin") !== url.origin
  )
    return reply(403);
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
  const abort = new AbortController();
  let headerTimer: ReturnType<typeof setTimeout> | undefined;
  try {
    // Never parse JSON here: Rust's u64 fields exceed JavaScript's Number precision.
    let body: Uint8Array<ArrayBuffer> | undefined;
    if (["POST", "PATCH"].includes(request.method) && request.body) {
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
    // Empty POST streams are emitted by SDKs without a Content-Type header.
    if (
      body?.byteLength &&
      !request.headers.get("content-type")?.startsWith("application/json")
    )
      return reply(415);
    headerTimer = setTimeout(
      () =>
        abort.abort(
          new DOMException("Core response timed out", "TimeoutError"),
        ),
      timeoutMs,
    );
    const response = await fetch(upstream, {
      method: request.method,
      body: body || undefined,
      headers: {
        ...(body ? { "Content-Type": "application/json" } : {}),
        ...(request.headers.has("range")
          ? { Range: request.headers.get("range")! }
          : {}),
        ...(apiKey ? { Authorization: `Bearer ${apiKey}` } : {}),
      },
      redirect: "error",
      signal: AbortSignal.any([request.signal, abort.signal]),
    });
    clearTimeout(headerTimer);
    headerTimer = undefined;
    return new Response(
      response.body ? idleBody(response.body, abort, timeoutMs) : null,
      {
        status: response.status,
        headers: {
          "Content-Type":
            response.headers.get("content-type") || "application/json",
          "Cache-Control": "no-store",
          ...Object.fromEntries(
            [
              "content-length",
              "content-range",
              "accept-ranges",
              "content-disposition",
              "x-content-type-options",
            ].flatMap((name) =>
              response.headers.has(name)
                ? [[name, response.headers.get(name)!]]
                : [],
            ),
          ),
        },
      },
    );
  } catch {
    return reply(502);
  } finally {
    clearTimeout(headerTimer);
  }
}

/** Bound actual bytes, including requests without a trustworthy Content-Length. */
export async function readBounded(
  request: Request,
  maximum: number,
): Promise<string> {
  const reader = request.body?.getReader();
  if (!reader) return "";
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > maximum) {
        await reader.cancel();
        throw new RangeError("Request too large");
      }
      chunks.push(value);
    }
  } finally {
    reader.releaseLock();
  }
  const data = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    data.set(chunk, offset);
    offset += chunk.length;
  }
  return new TextDecoder().decode(data);
}

// Reset the idle deadline for each requested chunk. Slow consumers apply
// backpressure without turning a progressing transfer into a total-time timeout.
function idleBody(
  body: ReadableStream<Uint8Array>,
  abort: AbortController,
  timeoutMs: number,
): ReadableStream<Uint8Array> {
  const reader = body.getReader();
  let released = false;
  const release = () => {
    if (!released) {
      released = true;
      reader.releaseLock();
    }
  };
  return new ReadableStream({
    async pull(controller) {
      const timer = setTimeout(
        () =>
          abort.abort(new DOMException("Core stream stalled", "TimeoutError")),
        timeoutMs,
      );
      try {
        const { done, value } = await reader.read();
        if (done) {
          controller.close();
          release();
        } else controller.enqueue(value);
      } catch (error) {
        controller.error(error);
        abort.abort(error);
        release();
      } finally {
        clearTimeout(timer);
      }
    },
    async cancel(reason) {
      abort.abort(reason);
      try {
        await reader.cancel(reason);
      } catch {
        /* The fetch may already be aborted. */
      }
      release();
    },
  });
}
