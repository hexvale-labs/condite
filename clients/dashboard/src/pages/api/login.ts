import { isLocale } from "../../i18n";
import type { APIRoute } from "astro";
import { coreOrigin } from "../../lib/server";
export const POST: APIRoute = async ({ request, url, cookies, redirect }) => {
  const requestedLocale = url.searchParams.get("locale");
  const locale =
    requestedLocale && isLocale(requestedLocale) ? requestedLocale : "en";
  const failedLogin = `/${locale}/login?error=1`;
  if (request.headers.get("origin") !== url.origin)
    return new Response(null, { status: 403 });
  if (Number(request.headers.get("content-length")) > 2048)
    return new Response(null, { status: 413 });
  try {
    // Byte-limit the form even when Content-Length is missing or incorrect.
    const reader = request.body?.getReader();
    if (!reader) return new Response(null, { status: 400 });
    const chunks: Uint8Array[] = [];
    let length = 0;
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      length += value.length;
      if (length > 2048) {
        await reader.cancel();
        return new Response(null, { status: 413 });
      }
      chunks.push(value);
    }
    const bytes = new Uint8Array(length);
    let offset = 0;
    for (const chunk of chunks) {
      bytes.set(chunk, offset);
      offset += chunk.length;
    }
    const form = new URLSearchParams(new TextDecoder().decode(bytes));
    const api_key = form.get("api_key");
    if (!api_key || api_key.length > 512) return redirect(failedLogin, 303);
    const response = await fetch(`${coreOrigin()}/v1/sessions`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ api_key }),
      signal: AbortSignal.timeout(5000),
      redirect: "error",
    });
    if (!response.ok) return redirect(failedLogin, 303);
    const session = (await response.json()) as {
      token: string;
      expires_at: string;
    };
    cookies.set("condite_session", session.token, {
      httpOnly: true,
      secure: url.protocol === "https:",
      sameSite: "lax",
      path: "/",
      expires: new Date(session.expires_at),
    });
    return redirect(`/${locale}/`, 303);
  } catch {
    return redirect(failedLogin, 303);
  }
};
