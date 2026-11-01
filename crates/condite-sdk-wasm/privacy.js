import init, {
  Client as RawClient,
  privacyAssess,
  privacyMediaPlan,
  privacyValidateProbe,
} from "./condite_sdk.js";
export { default } from "./condite_sdk.js";
import { createFfmpegBackend as rawFfmpegBackend } from "./ffmpeg-backend.js";
export function createFfmpegBackend(ffmpeg, options) {
  return rawFfmpegBackend(ffmpeg, mediaPlan, validateMediaProbe, options);
}

export class Client extends RawClient {
  async uploadSanitized(
    request,
    source,
    options = {},
    config = {},
    mediaBackend,
  ) {
    const size =
      source instanceof Blob ? BigInt(source.size) : BigInt(source.byteLength);
    if (request.size_bytes !== size)
      throw Object.assign(new Error("size_bytes does not match data"), {
        code: "input",
      });
    const file = await new MetadataSanitizer(config, mediaBackend).prepare(
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
export async function mediaPlan(data) {
  await init();
  return privacyMediaPlan(data);
}
export async function validateMediaProbe(data, config = {}) {
  await init();
  privacyValidateProbe(data, config);
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

export class MetadataSanitizer {
  #config;
  #media;
  #queue = Promise.resolve();
  constructor(config = {}, mediaBackend) {
    this.#config = validateConfig(config);
    this.#media = mediaBackend;
  }
  async assess(source) {
    const size =
      source instanceof Blob ? BigInt(source.size) : BigInt(source.byteLength);
    await init();
    const early = privacyAssess(
      size,
      new Uint8Array(),
      this.#config,
      !!this.#media,
    );
    if (early.reason === "too-large") return early;
    const header =
      source instanceof Blob
        ? new Uint8Array(await source.slice(0, 64).arrayBuffer())
        : source.subarray(0, 64);
    return privacyAssess(size, header, this.#config, !!this.#media);
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
  prepare(source, filename, mimeType, options = {}, { signal } = {}) {
    const snapshot = { ...options };
    return this.#serial(async () => {
      const assessment = await this.assess(source);
      if (assessment.reason === "too-large") throw new Error("too-large");
      if (signal?.aborted) throw new DOMException("Cancelled", "AbortError");
      let input = source;
      let retained;
      let mediaChanged = false;
      if (
        snapshot.stripMetadata &&
        ["mp3", "flac", "wav", "mp4", "mov"].includes(assessment.format)
      ) {
        if (!this.#media) throw new Error("backend-unavailable");
        const bytes =
          source instanceof Blob
            ? new Uint8Array(await source.arrayBuffer())
            : source;
        const cleaned = await this.#media.strip(bytes, this.#config, {
          signal,
        });
        mediaChanged =
          cleaned.data.byteLength !== bytes.byteLength ||
          cleaned.data.some((value, index) => value !== bytes[index]);
        input = cleaned.data;
        mimeType = cleaned.mimeType;
        retained = cleaned.retained;
        snapshot.stripMetadata = false;
      }
      const result = await this.#run(
        "prepare",
        input,
        { filename, mimeType, options: snapshot },
        signal,
        !!retained,
      );
      if (retained) {
        result.report.metadataStripped = true;
        result.report.retained = retained;
        result.report.changed ||= mediaChanged;
      }
      return result;
    });
  }
  strip(
    source,
    filename = "file",
    mimeType = "application/octet-stream",
    control,
  ) {
    return this.prepare(
      source,
      filename,
      mimeType,
      { stripMetadata: true },
      control,
    );
  }
  decrypt(source, passphrase, { signal } = {}) {
    return this.#serial(() =>
      this.#run("decrypt", source, { passphrase }, signal),
    );
  }
  async #run(operation, source, args, signal, remuxed = false) {
    const size =
      source instanceof Blob ? BigInt(source.size) : BigInt(source.byteLength);
    const stageConfig = remuxed
      ? {
          ...this.#config,
          maxInputBytes: this.#config.maxOutputBytes ?? 314572800n,
        }
      : this.#config;
    const max = stageConfig.maxInputBytes ?? 314572800n;
    if (size > max)
      throw Object.assign(new Error("too-large"), { code: "input" });
    if (signal?.aborted) throw new DOMException("Cancelled", "AbortError");
    const worker = new Worker(new URL("./privacy-worker.js", import.meta.url), {
      type: "module",
    });
    try {
      return await new Promise((resolve, reject) => {
        const abort = () => {
          cleanup();
          reject(new DOMException("Cancelled", "AbortError"));
        };
        const cleanup = () => {
          clearTimeout(timer);
          signal?.removeEventListener("abort", abort);
          worker.terminate();
        };
        const timer = setTimeout(() => {
          cleanup();
          reject(new Error("resource-limit"));
        }, 30000);
        signal?.addEventListener("abort", abort, { once: true });
        worker.onerror = () => {
          cleanup();
          reject(new Error("privacy-worker-failed"));
        };
        worker.onmessage = ({ data }) => {
          cleanup();
          if (data.error)
            reject(Object.assign(new Error(data.error), { code: "input" }));
          else resolve(data.result);
        };
        const input = source instanceof Blob ? source : source.slice();
        worker.postMessage(
          { operation, source: input, args, config: stageConfig },
          input instanceof Uint8Array ? [input.buffer] : [],
        );
      });
    } finally {
      worker.terminate();
    }
  }
}
