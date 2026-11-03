import { defineMiddleware } from "astro:middleware";
import { isLocale } from "./i18n";
import { coreOrigin } from "./lib/server";

export const onRequest = defineMiddleware(async (context, next) => {
  const render = async (): Promise<Response> => {
    context.locals.session = context.cookies.get("condite_session")?.value;
    const path = context.url.pathname;
    const segment = path.split("/")[1];
    const locale = segment && isLocale(segment) ? segment : "en";
    const login = `/${locale}/login`;
    if (
      path === login ||
      path === "/api/login" ||
      path === "/api/logout" ||
      path.startsWith("/_astro/") ||
      path === "/favicon.svg"
    )
      return next();
    if (!context.locals.session)
      return path.startsWith("/api/")
        ? new Response(null, { status: 401 })
        : context.redirect(login, 303);
    // Revalidate every SSR/proxy request. Core rechecks revocation and root rotation.
    try {
      const account = await fetch(`${coreOrigin()}/v1/account`, {
        headers: { Authorization: `Bearer ${context.locals.session}` },
        signal: AbortSignal.timeout(5000),
        redirect: "error",
      });
      if (account.status === 401) {
        context.cookies.delete("condite_session", { path: "/" });
        return path.startsWith("/api/")
          ? new Response(null, { status: 401 })
          : context.redirect(login, 303);
      }
      if (!account.ok) return next();
      // Role only; usage quantities continue through the generated bigint SDK.
      const identity = (await account.json()) as { role: string };
      context.locals.root = identity.role === "root";
    } catch {
      context.locals.root = false;
    }
    return next();
  };
  const response = await render();
  response.headers.set("Cache-Control", "no-store");
  return response;
});
