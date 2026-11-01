/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { join } from "node:path";

import type { IpcMainInvokeEvent } from "electron";

import { conditeMaxBytes } from "./limits";
import type {
    ConfirmResponse,
CreateShareRequest, DownloadResponse,
    HealthStatus,
ListSharesResponse, ListUploadsRequest,
    ListUploadsResponse,
    PresignRequest,
    PresignResponse,
Retention, Share, Upload, UploadResult } from "./wire";

interface SdkClient {
    createShare(id: string, request: CreateShareRequest): Promise<Share>;
    listShares(id: string, request: ListUploadsRequest): Promise<ListSharesResponse>;
    revokeShare(id: string, share: string): Promise<void>;
    updateRetention(id: string, request: Retention): Promise<Upload>;
    download(id: string): Promise<DownloadResponse>;
    health(): Promise<HealthStatus>;
    listUploads(request: ListUploadsRequest): Promise<ListUploadsResponse>;
    presign(request: PresignRequest): Promise<PresignResponse>;
    putUpload(signed: PresignResponse, bytes: Uint8Array): Promise<void>;
    confirm(id: string): Promise<ConfirmResponse>;
    delete(id: string): Promise<void>;
    upload(request: PresignRequest, bytes: Uint8Array): Promise<UploadResult>;
}

export type NativeResult<T> =
    | { ok: true; value: T }
    | {
          ok: false;
          error: {
              message: string;
              code?: string;
              status?: number;
              request_id?: string;
              stage?: string;
          };
      };
type WireConfirm = Omit<ConfirmResponse, "new_storage_size_bytes" | "upload"> & {
    new_storage_size_bytes: string;
    upload: Omit<Upload, "size_bytes"> & { size_bytes: string };
};

function client(base: string, apiKey?: string): SdkClient {
    const url = new URL(base);
    if (
        !["http:", "https:"].includes(url.protocol) ||
        url.username ||
        url.password
    )
        throw new Error("Use an HTTP(S) API base without credentials");
    // Variable require keeps the native addon out of esbuild. This folder travels with Vencord's dist.
    const sdkPath = join(__dirname, "condite-sdk", "index.cjs");
    const { Client } = require(sdkPath) as {
        Client: new (base: string, apiKey?: string) => SdkClient;
    };
    return new Client(url.href, apiKey || undefined);
}

async function result<T>(
    operation: () => Promise<T>
): Promise<NativeResult<T>> {
    try {
        return { ok: true, value: await operation() };
    } catch (cause) {
        const error = cause as Error & {
            code?: string;
            status?: number;
            request_id?: string;
            stage?: string;
        };
        return {
            ok: false,
            error: {
                message: error.message || "Condite request failed",
                code: error.code,
                status: error.status,
                request_id: error.request_id,
                stage: error.stage
            }
        };
    }
}

const confirmation = (value: ConfirmResponse): WireConfirm => ({
    ...value,
    upload: { ...value.upload, size_bytes: value.upload.size_bytes.toString() },
    new_storage_size_bytes: value.new_storage_size_bytes.toString()
});

export function health(_event: IpcMainInvokeEvent, base: string, apiKey?: string) {
    return result(() => client(base, apiKey).health());
}
export function listUploads(
    _event: IpcMainInvokeEvent,
    base: string,
    request: ListUploadsRequest,
    apiKey?: string
) {
    return result(async () => {
        const page = await client(base, apiKey).listUploads(request);
        return {
            ...page,
            uploads: page.uploads.map(row => ({
                ...row,
                size_bytes: row.size_bytes.toString()
            }))
        };
    });
}
export function presign(
    _event: IpcMainInvokeEvent,
    base: string,
    filename: string,
    mimeType: string,
    size: string,
    apiKey?: string,
    retention?: Retention
) {
    return result(() =>
        client(base, apiKey).presign({
            filename,
            mime_type: mimeType,
            size_bytes: BigInt(size),
            retention
        })
    );
}
export function putUpload(
    _event: IpcMainInvokeEvent,
    base: string,
    signed: PresignResponse,
    bytes: Uint8Array,
    apiKey?: string
) {
    return result(() => {
        if (
            !(bytes instanceof Uint8Array) ||
            bytes.byteLength > conditeMaxBytes
        )
            throw new Error("Condite's current maximum is 100 MiB per file");
        return client(base, apiKey).putUpload(signed, bytes);
    });
}
export function confirm(_event: IpcMainInvokeEvent, base: string, id: string, apiKey?: string) {
    return result(async () => confirmation(await client(base, apiKey).confirm(id)));
}
export function deleteUpload(
    _event: IpcMainInvokeEvent,
    base: string,
    id: string,
    apiKey?: string
) {
    return result(() => client(base, apiKey).delete(id));
}
export function upload(
    _event: IpcMainInvokeEvent,
    base: string,
    filename: string,
    mimeType: string,
    bytes: Uint8Array,
    apiKey?: string,
    retention?: Retention
) {
    return result(async () => {
        if (
            !(bytes instanceof Uint8Array) ||
            bytes.byteLength > conditeMaxBytes
        )
            throw new Error("Condite's current maximum is 100 MiB per file");
        const value = await client(base, apiKey).upload(
            {
                filename,
                mime_type: mimeType,
                size_bytes: BigInt(bytes.byteLength),
                retention
            },
            bytes
        );
        return { ...value, confirmation: confirmation(value.confirmation) };
    });
}

export function createShare(_event: IpcMainInvokeEvent, base: string, id: string, request: CreateShareRequest, apiKey?: string) {
    return result(() => client(base, apiKey).createShare(id, request));
}
export function listShares(_event: IpcMainInvokeEvent, base: string, id: string, request: ListUploadsRequest, apiKey?: string) {
    return result(() => client(base, apiKey).listShares(id, request));
}
export function revokeShare(_event: IpcMainInvokeEvent, base: string, id: string, share: string, apiKey?: string) {
    return result(() => client(base, apiKey).revokeShare(id, share));
}
export function updateRetention(_event: IpcMainInvokeEvent, base: string, id: string, request: Retention, apiKey?: string) {
    return result(async () => { const row = await client(base, apiKey).updateRetention(id, request); return { ...row, size_bytes: row.size_bytes.toString() }; });
}
export function download(_event: IpcMainInvokeEvent, base: string, id: string, apiKey?: string) {
    return result(() => client(base, apiKey).download(id));
}
