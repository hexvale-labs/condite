import type { APIRoute } from "astro";
import { proxyRequest } from "../../lib/proxy";
import { coreOrigin } from "../../lib/server";
export const ALL: APIRoute = ({ request, locals }) =>
  proxyRequest(request, coreOrigin(), locals.session);
