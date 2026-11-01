// SPDX-License-Identifier: GPL-3.0-or-later
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
        apiKeyPlaceholder: {
            type: OptionType.STRING,
            get displayName() {
                return t("apiKey");
            },
            get description() {
                return t("apiKey");
            },
            default: ""
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
    },
    {
        apiKeyPlaceholder: { disabled: () => true }
    }
);
