import {
  Client as NodeClient,
  MetadataSanitizer as NodeSanitizer,
  type PresignRequest as NodeRequest,
} from "../../crates/condite-sdk-node/dist/index.cjs";
import init, {
  Client as BrowserClient,
  MetadataSanitizer as BrowserSanitizer,
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

async function privacyTypes(source: Blob) {
  const native = new NodeSanitizer(
    { maxInputBytes: 314572800n },
    { ffmpegPath: "/example/ffmpeg", ffprobePath: "/example/ffprobe" },
  );
  const browser = new BrowserSanitizer({ maxInputBytes: 314572800n });
  const file = await browser.prepare(
    source,
    "example.png",
    "image/png",
    { stripMetadata: true, anonymizeFilename: true },
    { signal: new AbortController().signal },
  );
  const size: bigint = file.sizeBytes;
  const filename: string = (
    await native.prepareFile("/example/file.png", { stripMetadata: true })
  ).filename;
  return { size, filename };
}
void privacyTypes;
// @ts-expect-error Privacy byte limits require bigint.
new NodeSanitizer({ maxInputBytes: 300 });
