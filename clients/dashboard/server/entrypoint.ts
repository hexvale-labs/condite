// Astro 7's shared manifest must be configured before the Node adapter creates
// its app. Keep the adapter's standalone server, static assets and body limits.
import { manifest } from "virtual:astro:manifest";
import { allowedDomains } from "./origin";
import { dashboardConfig } from "./config";
const config = dashboardConfig();
process.env.HOST = config.host;
process.env.PORT = String(config.port);

manifest.allowedDomains = allowedDomains(config.origins);
const server = await import("@astrojs/node/server.js");
export const handler = server.handler;
export const startServer = server.startServer;
export const options = server.options;
