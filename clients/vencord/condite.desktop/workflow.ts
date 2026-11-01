/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

export interface PrivacyOptions {
    stripMetadata: boolean;
    anonymizeFilename: boolean;
    passphrase?: string;
}
export interface PrivacyReport {
    policyVersion: string;
    metadataStripped: boolean;
    filenameAnonymized: boolean;
    encrypted: boolean;
    changed: boolean;
    retained: string[];
}
export interface PreparedFile {
    data: Uint8Array;
    filename: string;
    mimeType: string;
    sizeBytes: bigint;
    report: PrivacyReport;
}
export interface Sdk {
    MetadataSanitizer: new (config: typeof limits, media?: { ffmpegPath: string; ffprobePath: string; }) => {
        prepare(data: Uint8Array, filename: string, mime: string, options: PrivacyOptions): Promise<PreparedFile>;
        decrypt(data: Uint8Array, passphrase: string): Promise<Uint8Array>;
    };
    Client: new (apiUrl: string) => {
        upload(request: { filename: string; mime_type: string; size_bytes: bigint; }, data: Uint8Array): Promise<{
            confirmation: { public_uri: string; expires_at: string; };
        }>;
    };
}
export const limits = {
    maxInputBytes: 300n * 1024n * 1024n,
    maxOutputBytes: 100n * 1024n * 1024n,
    maxInflatedBytes: 512n * 1024n * 1024n,
    maxEntryBytes: 64n * 1024n * 1024n,
    maxEntries: 10_000
};
export class OperationError extends Error {}
export function validateApiUrl(value: string): string {
    let url: URL;
    try { url = new URL(value); }
    catch { throw new OperationError("Set a valid Condite API URL in plugin settings."); }
    const local = ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
    if ((url.protocol !== "https:" && !(local && url.protocol === "http:")) || url.username || url.password || url.search || url.hash) {
        throw new OperationError("Use an HTTPS API URL, or HTTP on localhost, without credentials or query parameters.");
    }
    return url.toString().replace(/\/$/, "");
}
export async function uploadPrepared(
    sdk: Sdk, apiUrl: string, data: Uint8Array, filename: string,
    options: PrivacyOptions, media?: { ffmpegPath: string; ffprobePath: string; }
): Promise<{ filename: string; uri: string; expiresAt: string; report: PrivacyReport; }> {
    const baseUrl = validateApiUrl(apiUrl);
    let prepared: PreparedFile;
    try {
        prepared = await new sdk.MetadataSanitizer(limits, media).prepare(data, filename, "application/octet-stream", options);
        if (prepared.sizeBytes !== BigInt(prepared.data.byteLength) || prepared.sizeBytes > limits.maxOutputBytes) {
            throw new Error("invalid prepared size");
        }
    } catch {
        throw new OperationError("File preparation failed. Upload was not started. Check format support, limits and optional media tools.");
    }
    let result: Awaited<ReturnType<InstanceType<Sdk["Client"]>["upload"]>>;
    try {
        result = await new sdk.Client(baseUrl).upload({
            filename: prepared.filename,
            mime_type: prepared.mimeType,
            size_bytes: prepared.sizeBytes
        }, prepared.data);
    } catch {
        throw new OperationError("Upload could not be confirmed. Check the Condite server; a pending upload may remain.");
    }
    let uri: URL;
    try { uri = new URL(result.confirmation.public_uri); }
    catch { throw new OperationError("Upload was confirmed, but the server returned an invalid link."); }
    if (!["https:", "http:"].includes(uri.protocol) || uri.username || uri.password) {
        throw new OperationError("Upload was confirmed, but the server returned an unsupported link.");
    }
    return { filename: prepared.filename, uri: uri.toString(), expiresAt: result.confirmation.expires_at, report: prepared.report };
}

const retainedDescriptions: Record<string, string> = {
    "metadata-not-cleaned": "File metadata (cleaning was off)",
    "comments-and-revisions": "Comments and revision history",
    "notes-and-document-content": "Notes and document content",
    "external-links": "External links",
    "embedded-objects": "Embedded objects",
    "icc-profile": "Color profiles",
    "orientation": "Image orientation",
    "rendering-metadata": "Information needed to display the image",
    "annotations": "Annotations",
    "forms": "Form contents",
    "attachments": "File attachments",
    "embedded-image-metadata": "Metadata inside embedded images",
    "accessibility-and-page-content": "Accessibility information and page content",
    "pdf-a-conformance-not-guaranteed": "PDF/A compatibility is not guaranteed",
    "cooperative-limits-no-hard-memory-or-time-bound": "Processing limits are cooperative",
    "codec-bitstream-metadata-not-cleaned": "Metadata inside audio and video streams",
    "rotation-color-playback-structures": "Playback, orientation and color information",
    "ffmpeg-build-dependent-remux": "Media support depends on the installed tools"
};

export function retainedLabels(categories: readonly string[]): string[] {
    return [...new Set(categories.map(category => Object.hasOwn(retainedDescriptions, category) ? retainedDescriptions[category] : "Additional embedded content"))];
}
