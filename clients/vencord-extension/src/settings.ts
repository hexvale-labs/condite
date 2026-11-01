// SPDX-License-Identifier: GPL-3.0-or-later
import { definePluginSettings } from "@api/Settings";
import { OptionType } from "@utils/types";

export const settings = definePluginSettings(
    {
        apiBase: {
            type: OptionType.STRING,
            description: "Condite API base URL",
            default: "http://localhost:8080"
        },
        apiKeyPlaceholder: {
            type: OptionType.STRING,
            description: "API key (reserved for future authentication support)",
            default: ""
        },
        uploadBehavior: {
            type: OptionType.SELECT,
            description: "Default upload behavior",
            options: [
                {
                    label: "Always Condite",
                    value: "condite" as const,
                    default: true
                },
                {
                    label: "Discord within its limit, Condite for larger files",
                    value: "automatic" as const
                },
                { label: "Choose for each file", value: "choose" as const }
            ]
        },
        discordLimitMiB: {
            type: OptionType.NUMBER,
            description:
                "Discord limit override in MiB (0 uses Discord's live limit, then account/server fallbacks)",
            default: 0
        }
    },
    {
        apiKeyPlaceholder: { disabled: () => true }
    }
);
