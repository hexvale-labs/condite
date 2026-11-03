import { isLocale } from "../../i18n";
import type { APIRoute } from "astro";
import { coreOrigin } from "../../lib/server";
export const POST: APIRoute = async ({ request, url, cookies, redirect }) => {
  if (request.headers.get("origin") !== url.origin)
    return new Response(null, { status: 403 });
  const token = cookies.get("condite_session")?.value;
  if (token) {
    try {
      const response = await fetch(`${coreOrigin()}/v1/sessions/current`, {
        method: "DELETE",
        headers: { Authorization: `Bearer ${token}` },
        signal: AbortSignal.timeout(5000),
        redirect: "error",
      });
      if (!response.ok)
        return new Response("Logout unavailable; try again", { status: 502 });
    } catch {
      return new Response("Logout unavailable; try again", { status: 502 });
    }
  }
  cookies.delete("condite_session", { path: "/" });
  const requested = url.searchParams.get("locale");
  return redirect(
    `/${requested && isLocale(requested) ? requested : "en"}/login`,
    303,
  );
};
