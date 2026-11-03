/** Read only at server startup; never bake deployment domains into the build. */
export function allowedDomains(value: string | undefined) {
  const defaults = [{ hostname: "localhost" }, { hostname: "127.0.0.1" }];
  if (value === undefined) return defaults;
  const origins = value.split(",").map((entry) => {
    let origin: URL;
    try {
      origin = new URL(entry.trim());
    } catch {
      throw new Error("DASHBOARD_ORIGINS must contain comma-separated HTTP(S) origins");
    }
    if (!["http:", "https:"].includes(origin.protocol) || origin.username || origin.password ||
        origin.pathname !== "/" || origin.search || origin.hash || origin.hostname.includes("*")) {
      throw new Error("DASHBOARD_ORIGINS must contain HTTP(S) origins without credentials, paths, queries or fragments");
    }
    return {
      hostname: origin.hostname,
      protocol: origin.protocol.slice(0, -1) as "http" | "https",
      port: origin.port,
    };
  });
  return [...defaults, ...origins];
}
