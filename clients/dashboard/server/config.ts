import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { parseConfig, knownFields, httpUrl } from "@condite/config";

export interface DashboardConfig {
  core_url: string;
  origins: string;
  host: string;
  port: number;
}
export function dashboardConfig(): DashboardConfig {
  const index = process.argv.indexOf("--condite-config");
  const file = resolve(
    index >= 0 ? process.argv[index + 1]! : "dashboard.toml",
  );
  let source: string;
  try {
    source = readFileSync(file, "utf8");
  } catch {
    throw new Error("Cannot read dashboard TOML configuration");
  }
  const value = parseConfig(source, process.env);
  knownFields(value, ["core_url", "origins", "host", "port"]);
  const core_url = httpUrl(value.core_url);
  if (
    typeof value.origins !== "string" ||
    typeof value.host !== "string" ||
    typeof value.port !== "number" ||
    !Number.isInteger(value.port) ||
    value.port < 1 ||
    value.port > 65535
  )
    throw new Error("Invalid dashboard fields");
  return {
    core_url,
    origins: value.origins,
    host: value.host,
    port: value.port,
  };
}
