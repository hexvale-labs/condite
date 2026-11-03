import { parse, stringify } from "smol-toml";
export { stringify };

/** Parse first, interpolate values once. Never expose parser source snippets. */
export function parseConfig(
  source: string,
  environment: Record<string, string | undefined> = {},
): Record<string, unknown> {
  let value: Record<string, unknown>;
  try {
    value = parse(source, {
      integersAsBigInt: "asNeeded",
      unsafeKeyBehaviour: "throw",
    });
  } catch {
    throw new Error("Invalid TOML syntax");
  }
  function expand(value: unknown, path: string): unknown {
    if (typeof value === "string") {
      let output = "";
      for (let index = 0; index < value.length;) {
        if (value.startsWith("$${", index)) {
          output += "${";
          index += 3;
          continue;
        }
        if (!value.startsWith("${", index)) {
          output += value[index++];
          continue;
        }
        const end = value.indexOf("}", index + 2);
        if (end < 0)
          throw new Error(`${path}: unterminated environment reference`);
        const name = value.slice(index + 2, end);
        if (!/^[A-Za-z0-9_]+$/.test(name))
          throw new Error(`${path}: invalid environment reference`);
        if (environment[name] === undefined)
          throw new Error(`${path}: missing environment variable ${name}`);
        output += environment[name];
        index = end + 1;
      }
      return output;
    }
    if (Array.isArray(value))
      return value.map((item, index) => expand(item, `${path}[${index}]`));
    if (value && typeof value === "object")
      return Object.fromEntries(
        Object.entries(value).map(([key, item]) => [
          key,
          expand(item, `${path}.${key}`),
        ]),
      );
    return value;
  }
  return expand(value, "config") as Record<string, unknown>;
}
export function knownFields(
  config: Record<string, unknown>,
  fields: readonly string[],
) {
  if (Object.keys(config).some((key) => !fields.includes(key)))
    throw new Error("Unknown configuration field");
}
export function httpUrl(value: unknown): string {
  if (typeof value !== "string") throw new Error("Expected an HTTP(S) URL");
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    throw new Error("Invalid HTTP(S) URL");
  }
  if (
    !["http:", "https:"].includes(url.protocol) ||
    url.username ||
    url.password ||
    url.search ||
    url.hash
  )
    throw new Error("Invalid HTTP(S) URL");
  return value.replace(/\/$/, "");
}

export interface ClientConfig {
  api_url?: string;
  api_key?: string;
  retention_days?: number;
  upload_behavior?: "condite" | "automatic" | "choose";
  discord_limit_mib?: number;
  sdk_path?: string;
  ffmpeg_path?: string;
  ffprobe_path?: string;
}
export function clientConfig(
  source: string,
  environment: Record<string, string | undefined> = {},
): ClientConfig {
  const value = parseConfig(source, environment);
  knownFields(value, [
    "api_url",
    "api_key",
    "retention_days",
    "upload_behavior",
    "discord_limit_mib",
    "sdk_path",
    "ffmpeg_path",
    "ffprobe_path",
  ]);
  if (value.api_url !== undefined) httpUrl(value.api_url);
  for (const name of ["api_key", "sdk_path", "ffmpeg_path", "ffprobe_path"])
    if (value[name] !== undefined && typeof value[name] !== "string")
      throw new Error(`Invalid ${name}`);
  for (const [name, max] of [
    ["retention_days", 36500],
    ["discord_limit_mib", 102400],
  ] as const)
    if (
      value[name] !== undefined &&
      (typeof value[name] !== "number" ||
        !Number.isInteger(value[name]) ||
        (value[name] as number) < 0 ||
        (value[name] as number) > max)
    )
      throw new Error(`Invalid ${name}`);
  if (
    value.upload_behavior !== undefined &&
    !["condite", "automatic", "choose"].includes(
      value.upload_behavior as string,
    )
  )
    throw new Error("Invalid upload_behavior");
  return value as ClientConfig;
}
