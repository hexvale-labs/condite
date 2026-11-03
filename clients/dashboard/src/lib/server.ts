import { createRequire } from "node:module";
import type { Client as CoreClient } from "condite-sdk-node";
const { Client } = createRequire(import.meta.url)(
  "condite-sdk-node",
) as typeof import("condite-sdk-node");
import { dashboardConfig } from "../../server/config";
export function coreOrigin(): string {
  return dashboardConfig().core_url;
}
export function serverClient(token?: string): CoreClient {
  return new Client(coreOrigin(), token);
}
// The native SDK has no cancellation API; bound how long SSR waits for it.
export async function withSsrDeadline<T>(operation: Promise<T>): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      operation,
      new Promise<never>((_, reject) => {
        timer = setTimeout(
          () => reject(new Error("Core response timed out")),
          5000,
        );
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}
