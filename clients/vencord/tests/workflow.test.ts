// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, test } from "bun:test";
import { OperationError, retainedLabels, uploadPrepared, validateApiUrl, type PreparedFile, type Sdk } from "../condite.desktop/workflow";

const report = { policyVersion: "test", metadataStripped: true, filenameAnonymized: true, encrypted: true, changed: true, retained: ["document-content"] };

describe("Condite SDK boundary", () => {
    test("uploads only prepared bytes with their actual name, MIME and bigint length", async () => {
        const steps: string[] = [];
        const prepared: PreparedFile = { data: new Uint8Array([7, 8]), filename: "anonymous.age", mimeType: "application/octet-stream", sizeBytes: 2n, report };
        const options = { stripMetadata: true, anonymizeFilename: true, passphrase: "transient canary" };
        const sdk: Sdk = {
            MetadataSanitizer: class {
                async prepare(data: Uint8Array, filename: string, mime: string, received: typeof options) {
                    expect(data).toEqual(new Uint8Array([1]));
                    expect(filename).toBe("private-name.docx");
                    expect(mime).toBe("application/octet-stream");
                    expect(received).toEqual(options);
                    steps.push("prepare");
                    return prepared;
                }
                async decrypt() { return new Uint8Array(); }
            },
            Client: class {
                constructor(url: string, key?: string) { expect(url).toBe("https://condite.example"); expect(key).toBe("test-key"); }
                async upload(request: { filename: string; mime_type: string; size_bytes: bigint; }, data: Uint8Array) {
                    steps.push("upload");
                    expect(request).toEqual({ filename: "anonymous.age", mime_type: "application/octet-stream", size_bytes: 2n });
                    expect(data).toBe(prepared.data);
                    return { request_id: "id", confirmation: { upload: { id: "id" } } };
                }
                async createShare(id: string, request: unknown) { expect(id).toBe("id"); expect(request).toEqual({follows_upload:true,expires_at:null}); steps.push("share"); return {url:"https://storage.example/file",expires_at:"later"};
                }
            }
        };
        const result = await uploadPrepared(sdk, "https://condite.example/", new Uint8Array([1]), "private-name.docx", options, undefined, "test-key");
        expect(steps).toEqual(["prepare", "upload", "share"]);
        expect(result).toEqual({ filename: "anonymous.age", uri: "https://storage.example/file", expiresAt: "later", report });
    });

    test("failed preparation makes no upload call and exposes no exception contents", async () => {
        let calls = 0;
        const sdk: Sdk = {
            MetadataSanitizer: class {
                async prepare(): Promise<PreparedFile> { throw new Error("PRIVATE_METADATA_CANARY"); }
                async decrypt() { return new Uint8Array(); }
            },
            Client: class {
                async upload(): Promise<never> { calls++; throw new Error("unexpected upload"); }
                async createShare(): Promise<never> { throw new Error("unexpected sharing"); }
            }
        };
        let error = "";
        try { await uploadPrepared(sdk, "https://condite.example", new Uint8Array([1]), "file", { stripMetadata: true, anonymizeFilename: false }); }
        catch (e) { error = (e as Error).message; }
        expect(calls).toBe(0);
        expect(error).toContain("Upload was not started");
        expect(error).not.toContain("PRIVATE_METADATA_CANARY");
    });

    test("restricts API URLs to HTTPS or local development HTTP", () => {
        expect(validateApiUrl("http://127.0.0.1:3000/")).toBe("http://127.0.0.1:3000");
        for (const url of ["http://remote.example", "file:///tmp/api", "https://user:secret@condite.example", "https://condite.example?token=secret"]) {
            expect(() => validateApiUrl(url)).toThrow();
        }
    });
});

test("retained report categories use readable labels without exposing internal codes", () => {
    expect(retainedLabels(["comments-and-revisions", "icc-profile", "internal-code-not-for-ui", "another-unknown", "__proto__", "constructor"])).toEqual([
        "Comments and revision history", "Color profiles", "Additional embedded content"
    ]);
});

test("failed upload preserves its request ID and stage without exposing exception text", async () => {
    const sdk: Sdk = {
        MetadataSanitizer: class {
            async prepare() { return { data: new Uint8Array([1]), filename: "a", mimeType: "text/plain", sizeBytes: 1n, report }; }
            async decrypt() { return new Uint8Array(); }
        },
        Client: class {
            async upload(): Promise<never> { throw Object.assign(new Error("PRIVATE_CANARY"), {request_id:"pending-id",stage:"confirm"}); }
            async createShare(): Promise<never> { throw new Error("must not share failed upload"); }
        }
    };
    try {
        await uploadPrepared(sdk, "https://condite.example", new Uint8Array([1]), "a", {stripMetadata:false,anonymizeFilename:false});
        throw new Error("expected rejection");
    } catch (error) {
        expect(error).toBeInstanceOf(OperationError);
        expect((error as OperationError).request_id).toBe("pending-id");
        expect((error as OperationError).stage).toBe("confirm");
        expect((error as Error).message).not.toContain("PRIVATE_CANARY");
    }
});
