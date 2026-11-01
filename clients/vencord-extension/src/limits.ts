// SPDX-License-Identifier: GPL-3.0-or-later
// Fallbacks checked 2026-10-31. Prefer the live Discord helper; experiments can differ.
// https://support.discord.com/hc/en-us/articles/25444343291031
// https://support.discord.com/hc/en-us/articles/115000435108
export const MiB = 1024 ** 2;
export const accountLimitMiB: Record<number, number> = {
    0: 20, // Free
    1: 50, // Legacy Nitro Classic
    2: 1024, // Nitro (up to 1 GB)
    3: 50 // Nitro Basic
};
export const guildLimitMiB: Record<number, number> = {
    0: 0,
    1: 0,
    2: 50,
    3: 100
};
export const conditeMaxBytes = 100 * MiB;

export function fallbackLimit(premiumType: number, guildTier = 0) {
    return (
        Math.max(
            accountLimitMiB[premiumType] ?? accountLimitMiB[0],
            guildLimitMiB[guildTier] ?? 0
        ) * MiB
    );
}
