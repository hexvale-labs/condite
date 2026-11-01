export async function privacyContract(MetadataSanitizer, fixtures) {
  const assert = (ok, message) => {
    if (!ok) throw new Error(message);
  };
  for (const config of [
    { maxEntries: 1.5 },
    { maxInputBytes: 300 },
    { maxOutputBytes: 0n },
    { unexpected: true },
  ]) {
    let rejected = false;
    try {
      new MetadataSanitizer(config);
    } catch {
      rejected = true;
    }
    assert(rejected, "invalid config must reject without reads");
  }
  const sanitizer = new MetadataSanitizer();
  const bytes = new Uint8Array([97, 98, 99]);
  const renamed = await sanitizer.prepare(bytes, "Alice.txt", "text/plain", {
    anonymizeFilename: true,
  });
  assert(
    !renamed.filename.includes("Alice") && renamed.filename.endsWith(".txt"),
    "anonymous filename",
  );
  assert(
    renamed.sizeBytes === 3n && renamed.data[0] === 97,
    "bigint output size",
  );
  assert(!(await sanitizer.canStripMetadata(bytes)), "unsupported preflight");
  let rejected = false;
  try {
    await sanitizer.prepare(bytes, "a.txt", "text/plain", {
      stripMetadata: true,
    });
  } catch {
    rejected = true;
  }
  assert(rejected, "required stripping must fail closed");
  const small = new MetadataSanitizer({ maxInputBytes: 2n });
  assert(
    (await small.assess(new Blob([bytes]))).reason === "too-large",
    "Blob preflight size",
  );
  for (const [filename, encoded] of Object.entries(fixtures)) {
    const input = Uint8Array.from(atob(encoded), (c) => c.charCodeAt(0));
    assert(
      await sanitizer.canStripMetadata(new Blob([input])),
      `eligible ${filename}`,
    );
    const output = await sanitizer.strip(new Blob([input]), filename);
    assert(output.report.metadataStripped, `stripped ${filename}`);
    assert(
      !new TextDecoder().decode(output.data).includes("PRIVATE_CANARY"),
      `canary removed ${filename}`,
    );
    const encrypted = await sanitizer.prepare(
      input,
      filename,
      "application/octet-stream",
      {
        stripMetadata: true,
        anonymizeFilename: true,
        passphrase: "synthetic contract passphrase",
      },
    );
    assert(
      encrypted.mimeType === "application/octet-stream" &&
        encrypted.filename.endsWith(".age"),
      "encrypted type",
    );
    const decrypted = await sanitizer.decrypt(
      encrypted.data,
      "synthetic contract passphrase",
    );
    assert(
      !new TextDecoder().decode(decrypted).includes("PRIVATE_CANARY"),
      "clean before encrypt",
    );
    // One end-to-end crypto fixture is enough; other formats verify stripping.
    if (filename === Object.keys(fixtures)[0]) {
      let wrong = false;
      try {
        await sanitizer.decrypt(encrypted.data, "wrong passphrase");
      } catch {
        wrong = true;
      }
      assert(wrong, "wrong passphrase rejection");
    }
  }
  return "passed";
}
