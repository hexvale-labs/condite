import type { Client } from "condite-sdk-wasm";
let client: Promise<Client> | undefined;
export function browserClient(): Promise<Client> {
  return (client ??= import("condite-sdk-wasm")
    .then(async (sdk) => {
      await sdk.default();
      return new sdk.Client(`${window.location.origin}/api`);
    })
    .catch((error) => {
      client = undefined;
      throw error;
    }));
}
