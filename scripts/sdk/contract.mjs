export async function contract(Client, base) {
  const assert = (ok, message) => {
    if (!ok) throw new Error(message);
  };
  const client = new Client(`${base}/prefix`, "contract-key");
  assert((await client.health()).healthy, "health");
  const page = await client.listUploads({ limit: 2, after: "cursor&value" });
  assert(page.next_after === null, "null cursor");
  assert(
    page.uploads[0].size_bytes === 18446744073709551615n,
    "full u64 precision",
  );
  assert(page.uploads[0].status === "Confirmed", "status casing");
  const payload = {
    size_bytes: 3n,
    mime_type: "text/plain",
    filename: "a.txt",
  };
  const signed = await client.presign(payload);
  assert(
    signed.headers["content-type"] === "text/plain",
    "plain header object",
  );
  await client.putUpload(signed, new Uint8Array([97, 98, 99]));
  const confirmation = await client.confirm(signed.request_id);
  assert(
    confirmation.new_storage_size_bytes === 18446744073709551615n,
    "confirmation precision",
  );
  const uploaded = await client.upload(payload, new Uint8Array([97, 98, 99]));
  assert(uploaded.request_id === "id", "upload helper");
  try {
    await client.uploadSanitized(
      { ...payload, size_bytes: 4n },
      new Uint8Array([97, 98, 99]),
    );
    throw new Error("expected mismatch rejection");
  } catch (error) {
    assert(error.code === "input", "sanitized upload size mismatch");
  }
  try {
    await client.uploadSanitized(payload, new Uint8Array([97, 98, 99]), {
      stripMetadata: true,
    });
    throw new Error("expected required cleaning rejection");
  } catch (error) {
    assert(
      error.message.includes("unsupported-format"),
      "sanitized upload required cleaning",
    );
  }
  await client.delete("id");
  try {
    await client.confirm("missing");
    throw new Error("expected rejection");
  } catch (error) {
    assert(
      error.code === "http" &&
        error.status === 404 &&
        error.body === "missing upload",
      "HTTP error details",
    );
  }
  try {
    await client.upload(
      { ...payload, filename: "fail.txt" },
      new Uint8Array([97, 98, 99]),
    );
    throw new Error("expected rejection");
  } catch (error) {
    assert(
      error.request_id === "fail" &&
        error.status === 409 &&
        error.stage === "confirm",
      "partial upload identity",
    );
  }
  try {
    await client.upload(
      { ...payload, filename: "put-fail.txt" },
      new Uint8Array([97, 98, 99]),
    );
    throw new Error("expected rejection");
  } catch (error) {
    assert(
      error.request_id === "put-fail" &&
        error.stage === "put" &&
        error.status === 503,
      "PUT failure recovery",
    );
  }
  for (const size of [-1n, 18446744073709551616n]) {
    let rejected = false;
    try {
      await client.presign({ ...payload, size_bytes: size });
    } catch (error) {
      rejected = error.code === "input";
    }
    assert(rejected, "out-of-range bigint must reject with an input error");
  }
  return "passed";
}
