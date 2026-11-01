/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import type { PluginNative } from "@utils/types";

import { t } from "./i18n";
import { conditeMaxBytes } from "./limits";
import type { NativeResult } from "./native";
import { settings } from "./settings";
import type { CreateShareRequest,ListUploadsRequest, PresignResponse, Retention } from "./wire";

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
    const { apiKey } = settings.store;
    const row = <T extends { size_bytes: string }>(value: T) => ({ ...value, size_bytes: BigInt(value.size_bytes) });
    return {
        createShare: (id: string, request: CreateShareRequest) => unwrap(native().createShare(base, id, request, apiKey)),
        listShares: (id: string, request: ListUploadsRequest = {}) => unwrap(native().listShares(base, id, request, apiKey)),
        revokeShare: (id: string, share: string) => unwrap(native().revokeShare(base, id, share, apiKey)),
        updateRetention: async (id: string, request: Retention) => row(await unwrap(native().updateRetention(base, id, request, apiKey))),
        download: (id: string) => unwrap(native().download(base, id, apiKey)),
        health: () => unwrap(native().health(base, apiKey)),
        listUploads: async (request: ListUploadsRequest = {}) => {
            const page = await unwrap(native().listUploads(base, request, apiKey));
            return {
                ...page,
                uploads: page.uploads.map(row => ({
                    ...row,
                    size_bytes: BigInt(row.size_bytes)
                }))
            };
        },
        presign: (file: File, filename = file.name, retention?: Retention) =>
            unwrap(
                native().presign(
                    base,
                    filename,
                    file.type || "application/octet-stream",
                    BigInt(file.size).toString(),
                    apiKey, retention
                )
            ),
        putUpload: (signed: PresignResponse, bytes: Uint8Array) =>
            unwrap(native().putUpload(base, signed, bytes, apiKey)),
        confirm: async (id: string) => {
            const value = await unwrap(native().confirm(base, id, apiKey));
            return {
                ...value,
                upload: row(value.upload),
                new_storage_size_bytes: BigInt(value.new_storage_size_bytes)
            };
        },
        delete: (id: string) => unwrap(native().deleteUpload(base, id, apiKey)),
        upload: async (file: File, bytes: Uint8Array) => {
            const value = await unwrap(
                native().upload(
                    base,
                    file.name,
                    file.type || "application/octet-stream",
                    bytes, apiKey
                )
            );
            return {
                ...value,
                confirmation: {
                    ...value.confirmation,
                    upload: row(value.confirmation.upload),
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
    onStage: (stage: string) => void = () => {},
    retention?: Retention
) {
    if (file.size > conditeMaxBytes) throw new Error(t("fileLimit"));
    const sdk = createClient();
    onStage("preparing");
    const days = settings.store.retentionDays ?? 14;
    if (!retention && (!Number.isInteger(days) || days < 0 || days > 36500)) throw new Error(t("invalidExpiry"));
    const expiry = retention ?? { expires_at: days === 0 ? null : new Date(Date.now() + days * 86400000).toISOString() };
    const signed = await sdk.presign(file, filename, expiry);
    let stage = "put";
    try {
        onStage("uploading");
        await sdk.putUpload(signed, new Uint8Array(await file.arrayBuffer()));
        stage = "confirm";
        onStage("confirming");
        const value = await sdk.confirm(signed.request_id);
        stage = "share";
        const share = await sdk.createShare(signed.request_id, { follows_upload: true, expires_at: null });
        onStage("complete");
        return { ...value, public_uri: share.url, expires_at: share.expires_at };
    } catch (error) {
        // Retain pending records for recovery; the SDK/server know whether a PUT was completed.
        throw Object.assign(error as Error, {
            request_id: signed.request_id,
            stage
        });
    }
}
