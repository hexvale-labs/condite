// SPDX-License-Identifier: GPL-3.0-or-later
import type { PluginNative } from "@utils/types";

import { conditeMaxBytes } from "./limits";
import type { NativeResult } from "./native";
import { settings } from "./settings";
import type { ListUploadsRequest, PresignResponse } from "./wire";

function native() {
    const bridge = VencordNative.pluginHelpers.Condite as
        PluginNative<typeof import("./native")> | undefined;
    if (!bridge)
        throw new Error(
            "Condite needs desktop Vencord with its Electron SDK installed. Browser extensions are unsupported."
        );
    return bridge;
}
async function unwrap<T>(response: Promise<NativeResult<T>>) {
    const result = await response;
    if (!result.ok)
        throw Object.assign(new Error(result.error.message), result.error);
    return result.value;
}
export function createClient() {
    // Snapshot the endpoint: changing settings midway must not PUT/confirm against a different server.
    const base = settings.store.apiBase;
    return {
        health: () => unwrap(native().health(base)),
        listUploads: async (request: ListUploadsRequest = {}) => {
            const page = await unwrap(native().listUploads(base, request));
            return {
                ...page,
                uploads: page.uploads.map(row => ({
                    ...row,
                    size_bytes: BigInt(row.size_bytes)
                }))
            };
        },
        presign: (file: File, filename = file.name) =>
            unwrap(
                native().presign(
                    base,
                    filename,
                    file.type || "application/octet-stream",
                    BigInt(file.size).toString()
                )
            ),
        putUpload: (signed: PresignResponse, bytes: Uint8Array) =>
            unwrap(native().putUpload(base, signed, bytes)),
        confirm: async (id: string) => {
            const value = await unwrap(native().confirm(base, id));
            return {
                ...value,
                new_storage_size_bytes: BigInt(value.new_storage_size_bytes)
            };
        },
        delete: (id: string) => unwrap(native().deleteUpload(base, id)),
        upload: async (file: File, bytes: Uint8Array) => {
            const value = await unwrap(
                native().upload(
                    base,
                    file.name,
                    file.type || "application/octet-stream",
                    bytes
                )
            );
            return {
                ...value,
                confirmation: {
                    ...value.confirmation,
                    new_storage_size_bytes: BigInt(
                        value.confirmation.new_storage_size_bytes
                    )
                }
            };
        }
    };
}

export async function uploadFile(
    file: File,
    filename = file.name,
    onStage: (stage: string) => void = () => {}
) {
    if (file.size > conditeMaxBytes)
        throw new Error("Condite currently accepts files up to 100 MiB");
    const sdk = createClient();
    onStage("Preparing");
    const signed = await sdk.presign(file, filename);
    let stage = "put";
    try {
        onStage("Uploading");
        await sdk.putUpload(signed, new Uint8Array(await file.arrayBuffer()));
        stage = "confirm";
        onStage("Confirming");
        const value = await sdk.confirm(signed.request_id);
        onStage("Complete");
        return value;
    } catch (error) {
        // Retain pending records for recovery; the SDK/server know whether a PUT was completed.
        throw Object.assign(error as Error, {
            request_id: signed.request_id,
            stage
        });
    }
}
