/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import type { CloudUpload } from "@vencord/discord-types";

export type Provider = "condite" | "discord";
export type RoutingMode = "condite" | "automatic" | "choose";
export interface LinkEntry {
    name: string;
    url: string;
}

export function safeUrl(value: string) {
    const url = new URL(value);
    if (!["https:", "http:"].includes(url.protocol))
        throw new Error("Unsupported download URL");
    return url.href.replace(/[()]/g, c => (c === "(" ? "%28" : "%29"));
}

export function formatLinks(entries: LinkEntry[]) {
    const used = new Set<string>();
    return (
        "\n\n" +
        entries
            .map(({ name, url }) => {
                const base = name.replace(/[\r\n\t]/g, " ");
                let label = base;
                for (let n = 2; used.has(label); n++) label = `${base} (${n})`;
                used.add(label);
                label = label.replace(/[\\[\]*_~`|<>]/g, "\\$&");
                return `[${label}](${safeUrl(url)})`;
            })
            .join("\n")
    );
}

interface Dependencies {
    mode(): RoutingMode;
    limit(channelId: string): number;
    choose(
        uploads: CloudUpload[],
        failure: boolean
    ): Promise<Provider[] | null>;
    upload(upload: CloudUpload): Promise<string>;
    append(channelId: string, text: string): void | Promise<void>;
    report(filename: string, error: unknown): void;
}

export class UploadRouter {
    private bypass = new WeakSet<CloudUpload[]>();
    private offered = new WeakSet<CloudUpload>();
    private listeners = new Set<() => void>();
    private recoveries = new Set<Promise<void>>();
    private stopped = false;

    constructor(private deps: Dependencies) {}

    isDiscordBatch(uploads: CloudUpload[]) {
        return this.bypass.delete(uploads);
    }

    async route(
        uploads: CloudUpload[],
        nativeUpload: (uploads: CloudUpload[]) => Promise<unknown>
    ) {
        if (this.stopped || !uploads.length) return;
        const mode = this.deps.mode();
        const choices =
            mode === "choose"
                ? await this.deps.choose(uploads, false)
                : uploads.map(u =>
                      mode === "automatic" &&
                      u.item.file.size <= this.deps.limit(u.channelId)
                          ? "discord"
                          : "condite"
                  );
        if (!choices || this.stopped) return;
        await this.uploadCondite(
            uploads.filter((_, i) => choices[i] === "condite")
        );
        const discord = uploads.filter((_, i) => choices[i] === "discord");
        if (!discord.length || this.stopped) return;

        const pending = new Set<CloudUpload>();
        let scheduled = false;
        const failed = (upload: CloudUpload) => {
            if (
                this.stopped ||
                this.offered.has(upload) ||
                ["COMPLETED", "CANCELLED", "REMOVED_FROM_MSG_DRAFT"].includes(
                    upload.status
                )
            )
                return;
            this.offered.add(upload);
            pending.add(upload);
            if (scheduled) return;
            scheduled = true;
            const recovery = Promise.resolve()
                .then(async () => {
                    scheduled = false;
                    const files = [...pending];
                    pending.clear();
                    if (this.stopped) return;
                    const retry = await this.deps.choose(files, true);
                    if (retry && !this.stopped)
                        await this.uploadCondite(
                            files.filter(
                                (file, i) =>
                                    retry[i] === "condite" &&
                                    ![
                                        "COMPLETED",
                                        "CANCELLED",
                                        "REMOVED_FROM_MSG_DRAFT"
                                    ].includes(file.status)
                            )
                        );
                })
                .catch(error =>
                    this.deps.report("Discord upload recovery", error)
                );
            this.recoveries.add(recovery);
            void recovery.finally(() => this.recoveries.delete(recovery));
        };
        for (const upload of discord) {
            const cleanup = () => {
                upload.removeListener("error", onError);
                upload.removeListener("complete", cleanup);
                this.listeners.delete(cleanup);
            };
            const onError = () => {
                cleanup();
                failed(upload);
            };
            upload.once("error", onError);
            upload.once("complete", cleanup);
            this.listeners.add(cleanup);
        }
        this.bypass.add(discord);
        try {
            await nativeUpload(discord);
        } catch (error) {
            // A batch-level rejection need not identify a file. Never retry known successes/cancellations.
            for (const upload of discord) failed(upload);
            this.deps.report("Discord upload", error);
        } finally {
            this.bypass.delete(discord);
        }
    }

    private async uploadCondite(uploads: CloudUpload[]) {
        const completed = new Map<
            string,
            { upload: CloudUpload; url: string }[]
        >();
        for (const upload of uploads) {
            try {
                const url = safeUrl(await this.deps.upload(upload));
                const group = completed.get(upload.channelId) ?? [];
                group.push({ upload, url });
                completed.set(upload.channelId, group);
            } catch (error) {
                this.deps.report(upload.filename, error);
            }
        }
        for (const [channelId, group] of completed) {
            try {
                await this.deps.append(
                    channelId,
                    formatLinks(
                        group.map(({ upload, url }) => ({
                            name: upload.filename,
                            url
                        }))
                    )
                );
                for (const { upload } of group) upload.removeFromMsgDraft();
            } catch (error) {
                this.deps.report(
                    "Could not add links; use Uploads to recover them",
                    error
                );
            }
        }
    }

    async idle() {
        await Promise.all([...this.recoveries]);
    }

    stop() {
        this.stopped = true;
        for (const cleanup of this.listeners) cleanup();
        this.listeners.clear();
    }
}
