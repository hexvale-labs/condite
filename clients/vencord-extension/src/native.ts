// SPDX-License-Identifier: GPL-3.0-or-later
import { join } from "node:path";

import type { IpcMainInvokeEvent } from "electron";

import { conditeMaxBytes } from "./limits";
import type {
    ConfirmResponse,
    HealthStatus,
    ListUploadsRequest,
    ListUploadsResponse,
    PresignRequest,
    PresignResponse,
    UploadResult
} from "./wire";

interface SdkClient {
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
type WireConfirm = Omit<ConfirmResponse, "new_storage_size_bytes"> & {
    new_storage_size_bytes: string;
};

function client(base: string): SdkClient {
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
        Client: new (base: string) => SdkClient;
    };
    return new Client(url.href);
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
    new_storage_size_bytes: value.new_storage_size_bytes.toString()
});

export function health(_event: IpcMainInvokeEvent, base: string) {
    return result(() => client(base).health());
}
export function listUploads(
    _event: IpcMainInvokeEvent,
    base: string,
    request: ListUploadsRequest
) {
    return result(async () => {
        const page = await client(base).listUploads(request);
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
    size: string
) {
    return result(() =>
        client(base).presign({
            filename,
            mime_type: mimeType,
            size_bytes: BigInt(size)
        })
    );
}
export function putUpload(
    _event: IpcMainInvokeEvent,
    base: string,
    signed: PresignResponse,
    bytes: Uint8Array
) {
    return result(() => {
        if (
            !(bytes instanceof Uint8Array) ||
            bytes.byteLength > conditeMaxBytes
        )
            throw new Error("Condite's current maximum is 100 MiB per file");
        return client(base).putUpload(signed, bytes);
    });
}
export function confirm(_event: IpcMainInvokeEvent, base: string, id: string) {
    return result(async () => confirmation(await client(base).confirm(id)));
}
export function deleteUpload(
    _event: IpcMainInvokeEvent,
    base: string,
    id: string
) {
    return result(() => client(base).delete(id));
}
export function upload(
    _event: IpcMainInvokeEvent,
    base: string,
    filename: string,
    mimeType: string,
    bytes: Uint8Array
) {
    return result(async () => {
        if (
            !(bytes instanceof Uint8Array) ||
            bytes.byteLength > conditeMaxBytes
        )
            throw new Error("Condite's current maximum is 100 MiB per file");
        const value = await client(base).upload(
            {
                filename,
                mime_type: mimeType,
                size_bytes: BigInt(bytes.byteLength)
            },
            bytes
        );
        return { ...value, confirmation: confirmation(value.confirmation) };
    });
}
