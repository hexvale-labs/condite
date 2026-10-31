// SPDX-License-Identifier: GPL-3.0-or-later
import "./styles.css";

import { Button } from "@components/Button";
import definePlugin from "@utils/types";
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
import { errorText } from "./fileManager";
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
    authors: [],
    settings,
    patches: [uploadPatch, navigationPatch],
    renderUploadsNavigation,
    settingsAboutComponent: () => (
        <div className="condite-actions">
            <Button onClick={() => setPaneOpen(true)}>Open Uploads</Button>
            <p>
                Close settings to view the pane. API key support is reserved for
                a future SDK release. Current download links expire.
            </p>
        </div>
    ),
    toolboxActions: { "Open Uploads": () => setPaneOpen(true) },

    start() {
        router = new UploadRouter({
            mode: () => settings.store.uploadBehavior,
            limit: discordLimit,
            choose: chooseProviders,
            upload: async upload => {
                showToast({
                    text: `Uploading ${upload.filename} to Condite…`,
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
                    text: "Condite links added to the original channel draft",
                    variant: "success"
                });
            },
            report: (filename, error) =>
                showToast({
                    text: `${filename}: ${errorText(error)}`,
                    variant: "critical",
                    duration: 8000
                })
        });
        try {
            startNavigation();
        } catch (error) {
            showToast({
                text: `Could not mount Uploads: ${errorText(error)}`,
                variant: "critical"
            });
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
            showToast({
                text: `Attachments kept: ${errorText(error)}`,
                variant: "critical"
            });
        }
    }
});
