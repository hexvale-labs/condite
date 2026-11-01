// SPDX-License-Identifier: MIT
export type PresentedError = { message: string; raw: string };
export const errorCopy = {
  rawError: "Raw error",
  requestError: "Something went wrong while sending the request.",
  serverError: "The server couldn’t process the request.",
  unauthorizedError: "Authentication is required. Check your API key.",
  forbiddenError: "You don’t have permission to perform this action.",
  notFoundError: "The requested item could not be found.",
  conflictError:
    "The request conflicts with the current state. Refresh and try again.",
  rateLimitError: "Too many requests. Please try again later.",
  httpError: "The request failed (HTTP {status}).",
  unknownError: "Something went wrong. Please try again.",
};
function record(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object"
    ? (value as Record<string, unknown>)
    : {};
}
function parse(value: string): unknown {
  try {
    return JSON.parse(value);
  } catch {
    return undefined;
  }
}
function text(value: unknown): string {
  return typeof value === "string" ? value : "";
}
function rawError(value: unknown): string {
  const seen = new WeakSet<object>();
  return (
    JSON.stringify(
      value,
      (_key, item: unknown) => {
        if (typeof item === "bigint") return item.toString();
        if (item && typeof item === "object") {
          if (seen.has(item)) return "[Circular]";
          seen.add(item);
          if (item instanceof Error)
            return {
              ...item,
              name: item.name,
              message: item.message,
              stack: item.stack,
              cause: item.cause,
            };
        }
        return item;
      },
      2,
    ) ?? String(value)
  );
}
export function presentError(
  value: unknown,
  copy = errorCopy,
  depth = 0,
): PresentedError {
  const outer = record(value);
  const original = typeof value === "string" ? value : text(outer.message);
  const decoded = record(parse(original));
  const source = Object.keys(decoded).length ? decoded : outer;
  const message = text(source.message) || original;
  const http = /(?:^|\b)HTTP (\d{3}):\s*([\s\S]*)/.exec(message);
  const status =
    typeof source.status === "number"
      ? source.status
      : http
        ? Number(http[1])
        : undefined;
  const body = typeof source.body === "string" ? source.body : http?.[2];
  const fallback = status
    ? (
        {
          400: copy.requestError,
          401: copy.unauthorizedError,
          403: copy.forbiddenError,
          404: copy.notFoundError,
          409: copy.conflictError,
          429: copy.rateLimitError,
        } as Record<number, string>
      )[status] ||
      (status >= 500
        ? copy.serverError
        : copy.httpError.replace("{status}", String(status)))
    : copy.unknownError;
  let friendly = "";
  if (body?.trim()) {
    const json = parse(body);
    const problem = record(json);
    const title = text(problem.title);
    const detail =
      text(problem.detail) || text(problem.message) || text(problem.error);
    friendly =
      [title, detail === title ? "" : detail].filter(Boolean).join(": ") ||
      (json === undefined
        ? body
        : json &&
            typeof json === "object" &&
            !Array.isArray(json) &&
            !Object.keys(json).length
          ? fallback
          : typeof json === "string"
            ? json
            : JSON.stringify(json, null, 2));
  } else if (text(source.title) || text(source.detail)) {
    friendly = [text(source.title), text(source.detail)]
      .filter(Boolean)
      .join(": ");
  } else if (status) {
    friendly = fallback;
  } else {
    friendly =
      [text(source.title), text(source.detail)].filter(Boolean).join(": ") ||
      message;
    if (
      depth < 8 &&
      source.cause &&
      source.cause !== value &&
      (typeof record(source.cause).status === "number" ||
        /HTTP \d{3}:/.test(text(record(source.cause).message)))
    )
      friendly = `${friendly ? friendly + " " : ""}${presentError(source.cause, copy, depth + 1).message}`;
  }
  const raw =
    text(outer.raw) || (typeof value === "string" ? value : rawError(value));
  const bodyJson = body ? parse(body) : parse(original);
  return {
    message: friendly || copy.unknownError,
    raw:
      raw +
      (bodyJson !== undefined
        ? `\n\n${JSON.stringify(bodyJson, null, 2)}`
        : ""),
  };
}
