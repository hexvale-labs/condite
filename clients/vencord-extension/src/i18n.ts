/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { LocaleStore, useStateFromStores } from "@webpack/common";

// Populated from the MIT shared catalog by tooling.mjs sync.
import { direction, type Messages, messages, resolveLocale } from "./translations";

export function currentLocale() {
    return resolveLocale(LocaleStore?.locale ?? navigator.language);
}

export function t(key: keyof Messages): string {
    return messages(currentLocale())[key];
}

export function useTranslation() {
    const locale = useStateFromStores(
        LocaleStore ? [LocaleStore] : [],
        currentLocale
    );
    return { t: messages(locale), locale, dir: direction(locale) };
}
