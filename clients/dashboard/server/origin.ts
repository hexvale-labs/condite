/** Read only at server startup; never bake deployment domains into the build. */
export function allowedDomains(value: string | undefined) {
  const defaults = [{ hostname: "localhost" }, { hostname: "127.0.0.1" }];
  if (value === undefined) return defaults;
  let origin: URL;
  try {
    origin = new URL(value);
  } catch {
    throw new Error("DASHBOARD_ORIGIN must be an HTTP(S) origin");
  }
  if (!["http:", "https:"].includes(origin.protocol) || origin.username || origin.password ||
      origin.pathname !== "/" || origin.search || origin.hash || origin.hostname.includes("*")) {
    throw new Error("DASHBOARD_ORIGIN must be an HTTP(S) origin without credentials, path, query or fragment");
  }
  return [...defaults, {
    hostname: origin.hostname,
    protocol: origin.protocol.slice(0, -1) as "http" | "https",
    port: origin.port,
  }];
}
