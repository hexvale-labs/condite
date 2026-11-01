const {
  NativeClient,
  nativePrepare,
  nativeAssess,
  nativeDecrypt,
} = require("./condite-sdk.node");
function normalize(error) {
  try {
    const details = JSON.parse(error.message);
    return Object.assign(new Error(details.message), details);
  } catch {
    return Object.assign(error, {
      code: "input",
      status: null,
      body: null,
      request_id: null,
      stage: null,
    });
  }
}
class Client {
  #native;
  constructor(baseUrl) {
    try {
      this.#native = new NativeClient(baseUrl);
    } catch (error) {
      throw normalize(error);
    }
  }
  async #call(method, ...args) {
    try {
      return await this.#native[method](...args);
    } catch (error) {
      throw normalize(error);
    }
  }
  health() {
    return this.#call("health");
  }
  presign(request) {
    return this.#call("presign", request);
  }
  listUploads(request = {}) {
    return this.#call("listUploads", request);
  }
  confirm(id) {
    return this.#call("confirm", id);
  }
  delete(id) {
    return this.#call("delete", id);
  }
  putUpload(signed, data) {
    return this.#call("putUpload", signed, data);
  }
  upload(request, data) {
    return this.#call("upload", request, data);
  }
  async uploadSanitized(
    request,
    source,
    options = {},
    config = {},
    mediaPaths,
  ) {
    const size =
      source instanceof Blob ? BigInt(source.size) : BigInt(source.byteLength);
    if (request.size_bytes !== size)
      throw Object.assign(new Error("size_bytes does not match data"), {
        code: "input",
      });
    const file = await new MetadataSanitizer(config, mediaPaths).prepare(
      source,
      request.filename,
      request.mime_type,
      options,
    );
    return {
      upload: await this.upload(
        {
          filename: file.filename,
          mime_type: file.mimeType,
          size_bytes: file.sizeBytes,
        },
        file.data,
      ),
      privacy: file.report,
    };
  }
}
function validateConfig(config) {
  const byteFields = [
    "maxInputBytes",
    "maxOutputBytes",
    "maxInflatedBytes",
    "maxEntryBytes",
  ];
  for (const [key, value] of Object.entries(config)) {
    if (value === undefined) continue;
    if (key === "maxEntries") {
      if (!Number.isInteger(value) || value < 1 || value > 100000)
        throw new Error("invalid-config");
    } else if (
      !byteFields.includes(key) ||
      typeof value !== "bigint" ||
      value <= 0n ||
      value > 18446744073709551615n
    ) {
      throw new Error("invalid-config");
    }
  }
  return Object.fromEntries(
    Object.entries(config).filter(([, value]) => value !== undefined),
  );
}

class MetadataSanitizer {
  #config;
  #media;
  #queue = Promise.resolve();
  constructor(config = {}, mediaPaths) {
    this.#config = validateConfig(config);
    this.#media = mediaPaths;
  }
  async assess(source) {
    const size =
      source instanceof Blob ? BigInt(source.size) : BigInt(source.byteLength);
    const early = JSON.parse(
      nativeAssess(size, new Uint8Array(), this.#config, !!this.#media),
    );
    if (early.reason === "too-large") return early;
    const header =
      source instanceof Blob
        ? new Uint8Array(await source.slice(0, 64).arrayBuffer())
        : source.subarray(0, 64);
    return JSON.parse(nativeAssess(size, header, this.#config, !!this.#media));
  }
  async canStripMetadata(source) {
    return (await this.assess(source)).canAttempt;
  }
  #serial(work) {
    const job = this.#queue.then(work);
    this.#queue = job.then(
      () => undefined,
      () => undefined,
    );
    return job;
  }
  prepare(source, filename, mimeType, options = {}) {
    const snapshot = { ...options };
    return this.#serial(() =>
      this.#prepare(source, filename, mimeType, snapshot),
    );
  }
  async #prepare(source, filename, mimeType, options) {
    const size =
      source instanceof Blob ? BigInt(source.size) : BigInt(source.byteLength);
    const assessment = JSON.parse(
      nativeAssess(size, new Uint8Array(), this.#config, !!this.#media),
    );
    if (assessment.reason === "too-large") throw new Error("too-large");
    const bytes =
      source instanceof Blob
        ? new Uint8Array(await source.arrayBuffer())
        : source;
    const result = await nativePrepare(
      bytes,
      filename,
      mimeType,
      JSON.stringify(options),
      this.#config,
      this.#media,
    );
    const { reportJson, ...file } = result;
    return { ...file, report: JSON.parse(reportJson) };
  }
  async strip(
    source,
    filename = "file",
    mimeType = "application/octet-stream",
  ) {
    return this.prepare(source, filename, mimeType, { stripMetadata: true });
  }
  decrypt(source, passphrase) {
    return this.#serial(() => this.#decrypt(source, passphrase));
  }
  async #decrypt(source, passphrase) {
    const size =
      source instanceof Blob ? BigInt(source.size) : BigInt(source.byteLength);
    const assessment = JSON.parse(
      nativeAssess(size, new Uint8Array(), this.#config, !!this.#media),
    );
    if (assessment.reason === "too-large") throw new Error("too-large");
    const bytes =
      source instanceof Blob
        ? new Uint8Array(await source.arrayBuffer())
        : source;
    return nativeDecrypt(bytes, passphrase, this.#config);
  }
  async prepareFile(path, options = {}) {
    const fs = require("node:fs/promises");
    const file = await fs.open(path, "r");
    try {
      const size = (await file.stat({ bigint: true })).size;
      const assessment = JSON.parse(
        nativeAssess(size, new Uint8Array(), this.#config, !!this.#media),
      );
      if (assessment.reason === "too-large") throw new Error("too-large");
      if (!(await file.stat()).isFile()) throw new Error("unsupported-feature");
      const max = this.#config.maxInputBytes ?? 314572800n;
      const chunks = [];
      let total = 0n;
      while (true) {
        const chunk = Buffer.alloc(65536);
        const { bytesRead } = await file.read(chunk, 0, chunk.length, null);
        if (!bytesRead) break;
        total += BigInt(bytesRead);
        if (total > max) throw new Error("too-large");
        chunks.push(chunk.subarray(0, bytesRead));
      }
      const bytes = Buffer.concat(chunks);
      return this.prepare(
        bytes,
        require("node:path").basename(path),
        "application/octet-stream",
        options,
      );
    } finally {
      await file.close();
    }
  }
}
module.exports = { Client, MetadataSanitizer };
