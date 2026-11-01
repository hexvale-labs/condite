import type { APIRoute } from "astro";
import { proxyRequest } from "../../lib/proxy";
import { coreOrigin, coreApiKey } from "../../lib/server";
export const ALL: APIRoute = ({ request }) =>
  proxyRequest(request, coreOrigin(), coreApiKey());
