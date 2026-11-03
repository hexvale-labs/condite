/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import "./styles.css";

import { Button } from "@components/Button";
import definePlugin, { type PluginNative } from "@utils/types";
import type { CloudUpload } from "@vencord/discord-types";
import { find, findByPropsLazy } from "@webpack";
import {
    ChannelStore,
    DraftStore,
    DraftType,
    GuildStore,
    React,
    showToast,
    UserStore
} from "@webpack/common";

import { chooseProviders, closeChoosers } from "./chooser";
import { showError } from "./errors";
import { t, useTranslation } from "./i18n";
import { fallbackLimit, MiB } from "./limits";
import {
    renderUploadsNavigation,
    setPaneOpen,
    startNavigation,
    stopNavigation
} from "./navigation";
import { navigationPatch, uploadPatch } from "./patches";
import { UploadRouter } from "./routing";
import { uploadFile } from "./sdk";
import { settings } from "./settings";
import { ConfigEditor } from "./shared/ConfigEditor";

const DraftActions = findByPropsLazy("saveDraft", "clearDraft") as {
    saveDraft(channelId: string, content: string, draftType: number): void;
};
let liveLimit: ((user: unknown, guildId?: string) => number) | undefined;
let router: UploadRouter;

function discordLimit(channelId: string) {
    const override = settings.store.discordLimitMiB;
    if (Number.isFinite(override) && override > 0) return override * MiB;
    const user = UserStore.getCurrentUser();
    const guildId = ChannelStore.getChannel(channelId)?.guild_id;
    // Named exports are preferred; minified clients may not expose this helper by name.
    liveLimit ??= find(
        module => typeof module?.getUserMaxFileSize === "function"
    )?.getUserMaxFileSize;
    if (liveLimit) {
        try {
            const value = liveLimit(user, guildId);
            if (Number.isFinite(value) && value > 0) return value;
        } catch {
            /* Use the documented fallback for this client build. */
        }
    }
    return fallbackLimit(
        user?.premiumType ?? 0,
        guildId ? GuildStore.getGuild(guildId)?.premiumTier : 0
    );
}

export default definePlugin({
    name: "Condite",
    description:
        "Upload to Condite, add download links to channel drafts, and manage files inside Discord. Desktop Electron SDK required.",
    authors: [{ name: "Condite contributors", id: 955090007335530506n }],
    settings,
    patches: [uploadPatch, navigationPatch],
    renderUploadsNavigation,
    settingsAboutComponent: () => {
        const { t: text } = useTranslation();
        return (
            <div className="condite-actions">
                <ConfigEditor current={()=>({ api_url:settings.store.apiBase,api_key:settings.store.apiKey,retention_days:settings.store.retentionDays,upload_behavior:settings.store.uploadBehavior,discord_limit_mib:settings.store.discordLimitMiB })}
                    importFile={()=>(VencordNative.pluginHelpers.Condite as PluginNative<typeof import("./native")>).importSettings()}
                    apply={config=>{ if(config.api_url!==undefined)settings.store.apiBase=config.api_url;if(config.api_key!==undefined)settings.store.apiKey=config.api_key;if(config.retention_days!==undefined)settings.store.retentionDays=config.retention_days;if(config.upload_behavior!==undefined)settings.store.uploadBehavior=config.upload_behavior;if(config.discord_limit_mib!==undefined)settings.store.discordLimitMiB=config.discord_limit_mib; }}/>

                <Button onClick={() => setPaneOpen(true)}>
                    {text.openUploads}
                </Button>
                <p>{text.cleanupHint}</p><a href="https://github.com/hexvale-labs/condite/blob/main/AI-NOTICE.md" target="_blank" rel="noreferrer">{text.aiNotice}</a>
            </div>
        );
    },
    get toolboxActions() {
        return { [t("openUploads")]: () => setPaneOpen(true) };
    },

    start() {
        router = new UploadRouter({
            mode: () => settings.store.uploadBehavior,
            limit: discordLimit,
            choose: chooseProviders,
            upload: async upload => {
                showToast({
                    text: `${t("uploading")}: ${upload.filename}`,
                    variant: "default"
                });
                return (await uploadFile(upload.item.file, upload.filename))
                    .public_uri;
            },
            append: (channelId, suffix) => {
                const draft =
                    DraftStore.getDraft(channelId, DraftType.ChannelMessage) ||
                    "";
                DraftActions.saveDraft(
                    channelId,
                    draft + suffix,
                    DraftType.ChannelMessage
                );
                showToast({
                    text: t("draftAdded"),
                    variant: "success"
                });
            },
            report: (filename, error) =>
                showError(error, filename)
        });
        try {
            startNavigation();
        } catch (error) {
            showError(error, t("failedLoad"));
        }
    },
    stop() {
        router?.stop();
        closeChoosers();
        stopNavigation();
    },
    flux: { CHANNEL_SELECT: () => setPaneOpen(false) },
    isDiscordBatch(uploads: CloudUpload[]) {
        return router?.isDiscordBatch(uploads) ?? false;
    },
    async routeUploads(
        uploads: CloudUpload[],
        native: (uploads: CloudUpload[]) => Promise<unknown>
    ) {
        try {
            await router.route(uploads, native);
        } catch (error) {
            showError(error, t("keepAttachments"));
        }
    }
});
