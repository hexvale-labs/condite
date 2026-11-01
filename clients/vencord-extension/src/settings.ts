/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { definePluginSettings } from "@api/Settings";
import { OptionType } from "@utils/types";

import { t } from "./i18n";

export const settings = definePluginSettings(
    {
        apiBase: {
            type: OptionType.STRING,
            get displayName() {
                return t("apiBase");
            },
            get description() {
                return t("apiBase");
            },
            default: "http://localhost:8080"
        },
        apiKey: {
            type: OptionType.STRING,
            get displayName() {
                return t("apiKey");
            },
            get description() {
                return t("apiKeyDescription");
            },
            default: ""
        },
        retentionDays: {
            type: OptionType.NUMBER,
            get displayName() { return t("uploadRetention"); },
            get description() { return `${t("days")} (0 = ${t("never")})`; },
            default: 14
        },
        uploadBehavior: {
            type: OptionType.SELECT,
            get displayName() {
                return t("uploadBehavior");
            },
            get description() {
                return t("uploadBehavior");
            },
            options: [
                {
                    label: "Condite",
                    value: "condite" as const,
                    default: true
                },
                {
                    get label() {
                        return t("automatic");
                    },
                    value: "automatic" as const
                },
                {
                    get label() {
                        return t("chooseEach");
                    },
                    value: "choose" as const
                }
            ]
        },
        discordLimitMiB: {
            type: OptionType.NUMBER,
            get displayName() {
                return t("limitOverride");
            },
            get description() {
                return t("limitOverride");
            },
            default: 0
        }
    }
);
