// SPDX-License-Identifier: MIT
export type UptimeSample = { seconds: number; monotonic: number } | null;
export type HealthSample = {
  healthy: boolean;
  runtime: string;
  started_at: string;
};
export function sampleUptime(
  runtime: string,
  startedAt: string,
  wallNow: number,
  monotonicNow: number,
): UptimeSample {
  const match =
    /^P(?:(\d+)D)?(?:T(?:(\d+)H)?(?:(\d+)M)?(?:(\d+(?:\.\d+)?)S)?)?$/.exec(
      runtime,
    );
  let seconds =
    match && match.slice(1).some(Boolean)
      ? Number(match[1] || 0) * 86400 +
        Number(match[2] || 0) * 3600 +
        Number(match[3] || 0) * 60 +
        Number(match[4] || 0)
      : (wallNow - Date.parse(startedAt)) / 1000;
  if (
    !Number.isFinite(seconds) ||
    seconds < 0 ||
    seconds > Number.MAX_SAFE_INTEGER
  )
    return null;
  seconds = Math.floor(seconds);
  return { seconds, monotonic: monotonicNow };
}
export function elapsedSeconds(
  sample: UptimeSample,
  monotonicNow: number,
): number | null {
  return sample
    ? sample.seconds +
        Math.max(0, Math.floor((monotonicNow - sample.monotonic) / 1000))
    : null;
}
/** Months intentionally mean 30 days, not calendar months. */
export function splitUptime(seconds: number): number[] {
  let remaining = Math.max(0, Math.floor(seconds));
  return [2592000, 86400, 3600, 60, 1].map((unit) => {
    const value = Math.floor(remaining / unit);
    remaining %= unit;
    return value;
  });
}
export function formatUptime(seconds: number, locale: string): string {
  return splitUptime(seconds)
    .map((value, index) =>
      new Intl.NumberFormat(locale, {
        style: "unit",
        unit: ["month", "day", "hour", "minute", "second"][index]!,
        unitDisplay: "short",
      }).format(value),
    )
    .join(" ");
}
