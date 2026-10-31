import { Client } from "condite-sdk-node";
export function coreOrigin(): string {
  return (
    process.env.CONDITE_API_URL ||
    import.meta.env.CONDITE_API_URL ||
    "http://127.0.0.1:8080"
  );
}
export function serverClient(): Client {
  return new Client(coreOrigin());
}

// The native SDK has no cancellation API; bound how long SSR waits for it.
export async function withSsrDeadline<T>(operation: Promise<T>): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      operation,
      new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(new Error("Core response timed out")), 5000);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}
