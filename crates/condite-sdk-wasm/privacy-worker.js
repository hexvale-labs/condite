import init, { privacyPrepare, privacyDecrypt } from "./condite_sdk.js";
self.onmessage = async ({ data: { operation, source, args, config } }) => {
  try {
    await init();
    const bytes =
      source instanceof Blob
        ? new Uint8Array(await source.arrayBuffer())
        : source;
    const result =
      operation === "decrypt"
        ? privacyDecrypt(bytes, args.passphrase, config)
        : privacyPrepare(
            bytes,
            args.filename,
            args.mimeType,
            args.options,
            config,
          );
    args.passphrase = undefined;
    if (args.options) args.options.passphrase = undefined;
    const buffer =
      result instanceof Uint8Array ? result.buffer : result.data.buffer;
    self.postMessage({ result }, [buffer]);
  } catch (error) {
    // Only fixed SDK error codes leave the worker, never parser details or secrets.
    const allowed = [
      "too-large",
      "unsupported-format",
      "unsupported-feature",
      "malformed",
      "encrypted",
      "signed",
      "resource-limit",
      "backend-unavailable",
      "invalid-passphrase",
      "decryption-failed",
      "invalid-config",
    ];
    const code =
      typeof error.message === "string"
        ? error.message.replace(/^invalid input: /, "")
        : "";
    self.postMessage({
      error: allowed.includes(code) ? code : "processing-failed",
    });
  }
};
