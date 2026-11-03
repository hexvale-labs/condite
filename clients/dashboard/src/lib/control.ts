import type { ControlResponse } from "condite-sdk-node";
/** Modern Node preserves each numeric token's source; never round a u64 via Number. */
export function parseControlResponse(source: string): ControlResponse {
  return JSON.parse(
    source,
    (key: string, value: unknown, context?: { source?: string }) => {
      if (typeof value === "number" && key.endsWith("_bytes")) {
        if (context?.source && /^\d+$/.test(context.source))
          return BigInt(context.source);
        if (Number.isSafeInteger(value)) return BigInt(value);
        throw new Error("Runtime cannot decode exact byte quantities");
      }
      return value;
    },
  ) as ControlResponse;
}
