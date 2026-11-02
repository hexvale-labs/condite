import { expect, test } from "bun:test";
import {
  sampleUptime,
  elapsedSeconds,
  splitUptime,
  formatUptime,
} from "./uptime";
test("ISO duration sample ignores fractional seconds and advances monotonically despite wall skew", () => {
  const sample = sampleUptime("PT61.999S", "2099-01-01", 0, 100);
  expect(elapsedSeconds(sample, 2100)).toBe(63);
  expect(elapsedSeconds(sample, 0)).toBe(61);
});
test("invalid durations fall back to valid started_at; unknown and future dates stay unknown", () => {
  expect(
    elapsedSeconds(
      sampleUptime("fixture", "1970-01-01T00:00:00Z", 123000, 0),
      0,
    ),
  ).toBe(123);
  expect(sampleUptime("PT", "bad", 0, 0)).toBeNull();
  expect(sampleUptime("PT-1S", "2099-01-01", 0, 0)).toBeNull();
});
test("rollovers include 30-day months, days, hours, minutes and seconds", () => {
  expect(splitUptime(31 * 86400 + 3661)).toEqual([1, 1, 1, 1, 1]);
  expect(formatUptime(0, "en")).toContain("0 sec");
});

test("manual-install legacy bundle retains the shared time and audio source", async () => {
  for (const file of ["uptime.ts", "tick.ts", "uptime-dom.ts", "uptime.css", "LICENSE"]) {
    const current = await Bun.file(new URL(file, import.meta.url)).text();
    const bundled = await Bun.file(new URL(`../vencord/condite.desktop/shared/${file}`, import.meta.url)).text();
    expect(bundled).toBe(current);
  }
});
