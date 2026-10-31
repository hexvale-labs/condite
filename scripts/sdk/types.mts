import {
  Client as NodeClient,
  type PresignRequest as NodeRequest,
} from "../../crates/condite-sdk-node/dist/index.cjs";
import init, {
  Client as BrowserClient,
  type PresignRequest as BrowserRequest,
} from "../../crates/condite-sdk-wasm/dist/index.js";
const request: NodeRequest & BrowserRequest = {
  size_bytes: 3n,
  mime_type: "text/plain",
  filename: "a.txt",
};
async function check(client: NodeClient | BrowserClient) {
  await client.listUploads({ limit: 2 });
  const signed = await client.presign(request);
  await client.putUpload(signed, new Uint8Array(3));
  const result = await client.upload(request, new Uint8Array(3));
  const bytes: bigint = result.confirmation.new_storage_size_bytes;
  const cursor: string | null = (await client.listUploads()).next_after;
  return { bytes, cursor };
}
void init;
void check;
// @ts-expect-error The public API requires bigint sizes.
const invalid: NodeRequest = { ...request, size_bytes: 3 };
void invalid;
