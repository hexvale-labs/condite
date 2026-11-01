/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { mkdtemp, open, rename, rm, stat, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { basename, dirname, isAbsolute, join } from "node:path";

import { dialog, type IpcMainInvokeEvent } from "electron";

import { limits, OperationError, type PrivacyOptions, type Retention, type Sdk, uploadPrepared, validateApiUrl } from "./workflow";

type Reply<T> = ({ status: "ok"; } & T) | { status: "cancelled"; } | { status: "error"; message: string; request_id?: string; stage?: "put" | "confirm" | "share"; };
let busy = false;

function localSdk(): { sdk: Sdk; media?: { ffmpegPath: string; ffprobePath: string; }; } {
    const path = process.env.CONDITE_SDK_NODE_PATH;
    if (!path || !isAbsolute(path) || !path.endsWith(".cjs")) {
        throw new OperationError("Set CONDITE_SDK_NODE_PATH to the absolute local SDK dist/index.cjs path, then restart Discord.");
    }
    // Module paths come from the trusted desktop launch environment, never IPC.
    let sdk: Sdk;
    try { sdk = createRequire(process.execPath)(path); }
    catch { throw new OperationError("The local Condite SDK could not be loaded. Check its path, platform and native build."); }
    if (typeof sdk.Client !== "function" || typeof sdk.MetadataSanitizer !== "function") {
        throw new OperationError("Rebuild the local SDK with MetadataSanitizer support.");
    }
    const ffmpegPath = process.env.CONDITE_FFMPEG_PATH;
    const ffprobePath = process.env.CONDITE_FFPROBE_PATH;
    if ((ffmpegPath || ffprobePath) && (!ffmpegPath || !ffprobePath || !isAbsolute(ffmpegPath) || !isAbsolute(ffprobePath))) {
        throw new OperationError("Set both CONDITE_FFMPEG_PATH and CONDITE_FFPROBE_PATH to absolute local paths, or leave both unset.");
    }
    return { sdk, media: ffmpegPath && ffprobePath ? { ffmpegPath, ffprobePath } : undefined };
}

function privacyOptions(value: PrivacyOptions): PrivacyOptions {
    if (!value || typeof value.stripMetadata !== "boolean" || typeof value.anonymizeFilename !== "boolean"
        || (value.passphrase !== undefined && (typeof value.passphrase !== "string" || value.passphrase.length === 0 || value.passphrase.length > 4096))) {
        throw new OperationError("Invalid privacy options.");
    }
    return { stripMetadata: value.stripMetadata, anonymizeFilename: value.anonymizeFilename, passphrase: value.passphrase };
}

async function readSelectedFile(path: string): Promise<Uint8Array> {
    const file = await open(path, "r");
    try {
        const stat = await file.stat({ bigint: true });
        if (!stat.isFile() || stat.size > limits.maxInputBytes) {
            throw new OperationError("Choose a regular file no larger than 300 MiB.");
        }
        const chunks: Buffer[] = [];
        let total = 0n;
        // Also limit actual reads if a file changes after stat.
        while (true) {
            const chunk = Buffer.alloc(64 * 1024);
            const { bytesRead } = await file.read(chunk);
            if (bytesRead === 0) break;
            total += BigInt(bytesRead);
            if (total > limits.maxInputBytes) throw new OperationError("The selected file exceeded 300 MiB.");
            chunks.push(chunk.subarray(0, bytesRead));
        }
        return Buffer.concat(chunks);
    } finally { await file.close(); }
}

function failure(error: unknown): { status: "error"; message: string; request_id?: string; stage?: "put" | "confirm" | "share"; } {
    return error instanceof OperationError
        ? { status: "error", message: error.message, request_id: error.request_id, stage: error.stage }
        : { status: "error", message: "The local operation failed. Check the selected file and desktop setup." };
}

function apiCredentials(apiUrl: string, suppliedKey?: string) {
    const destination = validateApiUrl(apiUrl);
    if (suppliedKey) {
        if (typeof suppliedKey !== "string" || suppliedKey.length > 4096 || /[\r\n]/.test(suppliedKey)) throw new OperationError("Invalid API key.");
        return { destination, apiKey: suppliedKey };
    }
    const apiKey = process.env.CONDITE_API_KEY;
    if (!apiKey || !process.env.CONDITE_API_URL || validateApiUrl(process.env.CONDITE_API_URL) !== destination) {
        throw new OperationError("Set the API key in Condite settings, or supply CONDITE_API_KEY with matching CONDITE_API_URL in the desktop environment.");
    }
    return { destination, apiKey };
}

export async function chooseAndUpload(_event: IpcMainInvokeEvent, apiUrl: string, value: PrivacyOptions, suppliedKey?: string, retention?: Retention): Promise<Reply<Awaited<ReturnType<typeof uploadPrepared>>>> {
    if (busy) return { status: "error", message: "A Condite operation is already running." };
    busy = true;
    let options: PrivacyOptions | undefined;
    try {
        if (typeof apiUrl !== "string" || apiUrl.length > 2048) throw new OperationError("Invalid Condite API URL.");
        const { apiKey } = apiCredentials(apiUrl, suppliedKey);
        options = privacyOptions(value);
        const { sdk, media } = localSdk();
        const selected = await dialog.showOpenDialog({ title: "Choose a file for Condite", properties: ["openFile"] });
        if (selected.canceled || selected.filePaths.length !== 1) return { status: "cancelled" };
        const path = selected.filePaths[0];
        const data = await readSelectedFile(path);
        return { status: "ok", ...await uploadPrepared(sdk, apiUrl, data, basename(path), options, media, apiKey, retention) };
    } catch (error) { return failure(error); }
    finally {
        if (options) options.passphrase = undefined;
        if (value) value.passphrase = undefined;
        busy = false;
    }
}

export async function chooseDecryptAndSave(_event: IpcMainInvokeEvent, passphrase: string): Promise<Reply<{}>> {
    if (busy) return { status: "error", message: "A Condite operation is already running." };
    busy = true;
    try {
        if (typeof passphrase !== "string" || passphrase.length === 0 || passphrase.length > 4096) throw new OperationError("Enter a passphrase.");
        const { sdk, media } = localSdk();
        const selected = await dialog.showOpenDialog({ title: "Choose an encrypted Condite file", properties: ["openFile"] });
        if (selected.canceled || selected.filePaths.length !== 1) return { status: "cancelled" };
        const data = await readSelectedFile(selected.filePaths[0]);
        let decrypted: Uint8Array;
        try { decrypted = await new sdk.MetadataSanitizer(limits, media).decrypt(data, passphrase); }
        catch { throw new OperationError("Decryption failed. Check the passphrase and encrypted file."); }
        passphrase = "";
        try {
            const target = await dialog.showSaveDialog({ title: "Save decrypted file", defaultPath: "decrypted-file", properties: ["showOverwriteConfirmation"] });
            if (target.canceled || !target.filePath) return { status: "cancelled" };
            const original = await stat(selected.filePaths[0], { bigint: true });
            const existing = await stat(target.filePath, { bigint: true }).catch(() => undefined);
            if (existing && existing.dev === original.dev && existing.ino === original.ino) {
                throw new OperationError("Choose a different destination to keep the encrypted original.");
            }
            // Replace atomically with a newly private inode; mode alone would
            // leave an existing destination's permissive permissions unchanged.
            const staging = await mkdtemp(join(dirname(target.filePath), ".condite-decrypt-"));
            try {
                const plaintext = join(staging, "plaintext");
                await writeFile(plaintext, decrypted, { mode: 0o600, flag: "wx" });
                await rename(plaintext, target.filePath);
            } finally { await rm(staging, { recursive: true, force: true }); }
            return { status: "ok" };
        } finally { decrypted.fill(0); }
    } catch (error) { return failure(error); }
    finally { passphrase = ""; busy = false; }
}

// Explicit operation allowlist keeps renderer input away from arbitrary SDK methods.
export async function manage(_event: IpcMainInvokeEvent, apiUrl: string, apiKey: string, operation: "health" | "listUploads" | "confirm" | "delete" | "download" | "createShare" | "listShares" | "revokeShare" | "updateRetention", id = "", request: unknown = {}) {
    try {
        const credentials = apiCredentials(apiUrl, apiKey);
        const { sdk } = localSdk();
        type Manager = {
            health(): Promise<unknown>; listUploads(request: unknown): Promise<unknown>; confirm(id: string): Promise<unknown>;
            delete(id: string): Promise<unknown>; download(id: string): Promise<unknown>; createShare(id: string, request: unknown): Promise<unknown>;
            listShares(id: string, request: unknown): Promise<unknown>; revokeShare(id: string, share: string): Promise<unknown>; updateRetention(id: string, request: unknown): Promise<unknown>;
        };
        const client = new sdk.Client(credentials.destination, credentials.apiKey) as unknown as Manager;
        let value: unknown;
        switch (operation) {
            case "health": value = await client.health(); break;
            case "listUploads": value = await client.listUploads(request); break;
            case "confirm": value = await client.confirm(id); break;
            case "delete": value = await client.delete(id); break;
            case "download": value = await client.download(id); break;
            case "createShare": value = await client.createShare(id, request); break;
            case "listShares": value = await client.listShares(id, request); break;
            case "revokeShare": if (typeof request !== "string") throw new OperationError("Invalid share ID."); value = await client.revokeShare(id, request); break;
            case "updateRetention": value = await client.updateRetention(id, request); break;
            default: throw new OperationError("Unsupported Condite operation.");
        }
        return { status: "ok" as const, value: value === undefined ? null : JSON.parse(JSON.stringify(value, (_key, item) => typeof item === "bigint" ? item.toString() : item)) };
    } catch (cause) { return { status: "error" as const, message: (cause as Error).message || "Condite request failed." }; }
}
