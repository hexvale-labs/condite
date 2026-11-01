// Supply a dedicated, already-loaded @ffmpeg/ffmpeg instance and the matching
// Rust bindings. This module never loads a binary or chooses an asset URL.
const queues = new WeakMap();
const fail = (code) => Object.assign(new Error(code), { code: "input" });
const byteLimit = (value, fallback) => {
  if (typeof value === "number" && !Number.isSafeInteger(value))
    throw fail("invalid-config");
  const limit = BigInt(value ?? fallback);
  if (limit <= 0n) throw fail("invalid-config");
  return limit;
};

export function createFfmpegBackend(
  ffmpeg,
  rawPlan,
  rawValidateProbe,
  { timeoutMs = 30000, legacyProbeExitCode = false, reload } = {},
) {
  if (
    typeof legacyProbeExitCode !== "boolean" ||
    (reload !== undefined && typeof reload !== "function")
  )
    throw fail("invalid-config");
  if (legacyProbeExitCode && typeof reload !== "function")
    throw fail("invalid-config");
  if (!Number.isInteger(timeoutMs) || timeoutMs <= 0 || timeoutMs > 30000)
    throw fail("invalid-config");
  return {
    strip(data, config = {}, { signal } = {}) {
      if (
        !ffmpeg ||
        (typeof ffmpeg !== "object" && typeof ffmpeg !== "function")
      )
        return Promise.reject(fail("backend-unavailable"));
      const previous = queues.get(ffmpeg) ?? Promise.resolve();
      const next = previous
        .catch(() => {})
        .then(() => processMedia(data, config, signal));
      queues.set(
        ffmpeg,
        next.then(
          () => {},
          () => {},
        ),
      );
      return next;
    },
  };

  async function processMedia(data, config, signal) {
    if (
      !ffmpeg?.loaded ||
      typeof ffmpeg.ffprobe !== "function" ||
      typeof rawPlan !== "function" ||
      typeof rawValidateProbe !== "function"
    )
      throw fail("backend-unavailable");
    if (!(data instanceof Uint8Array)) throw fail("malformed");
    if (BigInt(data.byteLength) > byteLimit(config.maxInputBytes, 314572800))
      throw fail("too-large");
    const maxOutput = byteLimit(config.maxOutputBytes, 314572800);
    const maxProbe = byteLimit(config.maxEntryBytes, 67108864);
    if (signal?.aborted) throw new DOMException("Cancelled", "AbortError");
    const supplied = await rawPlan(data);
    if (signal?.aborted) throw new DOMException("Cancelled", "AbortError");
    const plan = typeof supplied === "string" ? JSON.parse(supplied) : supplied;
    if (
      !["mp3", "flac", "wav", "mp4", "mov"].includes(plan.extension) ||
      !Array.isArray(plan.args) ||
      !plan.args.every((arg) => typeof arg === "string")
    )
      throw fail("unsupported-feature");
    const directory = `condite-${globalThis.crypto.randomUUID()}`;
    const input = `${directory}/input.${plan.extension}`;
    const output = `${directory}/output.${plan.extension}`;
    const probeFile = `${directory}/probe.json`;
    const demuxer = plan.extension === "mp4" ? "mov" : plan.extension;
    const args = plan.args.map((arg) =>
      arg === `input.${plan.extension}`
        ? input
        : arg === `output.${plan.extension}`
          ? output
          : arg,
    );
    const deadline = Date.now() + timeoutMs;
    let timer;
    let abort;
    let interruption;
    const fresh = async (files) => {
      ffmpeg.terminate();
      await reload(ffmpeg);
      if (interruption) {
        ffmpeg.terminate();
        throw interruption;
      }
      if (!ffmpeg.loaded) throw fail("backend-unavailable");
      await ffmpeg.createDir(directory);
      for (const [path, bytes] of files)
        await ffmpeg.writeFile(path, bytes.slice());
    };
    const interrupted = new Promise((_, reject) => {
      timer = setTimeout(() => {
        interruption = fail("resource-limit");
        ffmpeg.terminate();
        reject(interruption);
      }, timeoutMs);
      abort = () => {
        interruption = new DOMException("Cancelled", "AbortError");
        ffmpeg.terminate();
        reject(interruption);
      };
      signal?.addEventListener("abort", abort, { once: true });
    });
    const probe = async (path) => {
      const probeArgs = [
        "-v",
        "error",
        "-max_alloc",
        maxProbe.toString(),
        "-protocol_whitelist",
        "file,pipe",
        "-f",
        demuxer,
      ];
      if (demuxer === "mov")
        probeArgs.push("-enable_drefs", "0", "-use_absolute_path", "0");
      probeArgs.push(
        "-show_error",
        "-show_streams",
        "-show_packets",
        "-show_entries",
        "stream:packet=stream_index:packet_side_data",
        "-of",
        "json",
        path,
        "-o",
        probeFile,
      );
      // core 0.12.10 discards ffprobe's successful return value (upstream #817).
      // Its opt-in compatibility path requires a fresh engine and validated JSON.
      const code = await ffmpeg.ffprobe(
        probeArgs,
        Math.max(1, deadline - Date.now()),
      );
      if (code !== 0 && !(legacyProbeExitCode && code === -1))
        throw fail("unsupported-feature");
      const json = await ffmpeg.readFile(probeFile);
      const bytes =
        typeof json === "string" ? new TextEncoder().encode(json) : json;
      // ffmpeg.wasm lacks a bounded read API: these are acceptance limits, not a
      // hard bound on its worker heap or virtual filesystem allocations.
      if (
        BigInt(bytes.byteLength) > maxProbe ||
        bytes.byteLength > 8 * 1024 * 1024
      )
        throw fail("resource-limit");
      let document;
      try {
        document = JSON.parse(new TextDecoder().decode(bytes));
      } catch {
        throw fail("malformed");
      }
      if (
        document.error ||
        !Array.isArray(document.streams) ||
        !Array.isArray(document.packets)
      )
        throw fail("malformed");
      await rawValidateProbe(bytes, config);
    };
    try {
      return await Promise.race([
        interrupted,
        (async () => {
          if (reload) await fresh([[input, data]]);
          else {
            await ffmpeg.createDir(directory);
            await ffmpeg.writeFile(input, data.slice());
          }
          await probe(input);
          if (reload) await fresh([[input, data]]);
          if (
            (await ffmpeg.exec(
              ["-max_alloc", maxProbe.toString(), ...args],
              Math.max(1, deadline - Date.now()),
            )) !== 0
          )
            throw fail("unsupported-feature");
          const result = await ffmpeg.readFile(output);
          if (!(result instanceof Uint8Array)) throw fail("malformed");
          if (BigInt(result.byteLength) > maxOutput)
            throw fail("resource-limit");
          if (reload) await fresh([[output, result]]);
          await probe(output);
          return {
            data: result,
            mimeType: plan.mimeType,
            retained: plan.retained,
          };
        })(),
      ]);
    } finally {
      clearTimeout(timer);
      signal?.removeEventListener("abort", abort);
      for (const file of [input, output, probeFile]) {
        try {
          await ffmpeg.deleteFile(file);
        } catch {
          /* Termination discards the filesystem. */
        }
      }
      try {
        await ffmpeg.deleteDir(directory);
      } catch {
        /* May already be terminated. */
      }
    }
  }
}
