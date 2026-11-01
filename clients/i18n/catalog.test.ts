import { test, expect } from "bun:test";
import { locales, messages, en, resolveLocale, direction } from "./index";

test("all 20 catalogs contain exactly the English keys and nonempty messages", () => {
  expect(locales).toHaveLength(20);
  for (const locale of locales) {
    const catalog = messages(locale);
    expect(Object.keys(catalog).sort()).toEqual(Object.keys(en).sort());
    for (const value of Object.values(catalog))
      expect(value.trim().length).toBeGreaterThan(0);
  }
});

test("Discord and browser locale variants resolve to supported defaults", () => {
  for (const [input, expected] of Object.entries({
    "en-US": "en",
    "es-ES": "es",
    "es-419": "es",
    pt: "pt-BR",
    "pt-PT": "pt-BR",
    "zh-TW": "zh-CN",
    "zh-Hans": "zh-CN",
    no: "nb",
    "nb-NO": "nb",
    fr_FR: "fr",
    unsupported: "en",
  })) {
    expect(resolveLocale(input)).toBe(expected);
  }
  expect(resolveLocale(undefined)).toBe("en");
  expect(direction("ar")).toBe("rtl");
  expect(direction("ja")).toBe("ltr");
});
