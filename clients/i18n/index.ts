// SPDX-License-Identifier: MIT
import en from "./en.json";
import pl from "./pl.json";
import es from "./es.json";
import fr from "./fr.json";
import de from "./de.json";
import zh_CN from "./zh-CN.json";
import ja from "./ja.json";
import pt_BR from "./pt-BR.json";
import ar from "./ar.json";
import hi from "./hi.json";
import ko from "./ko.json";
import nl from "./nl.json";
import it from "./it.json";
import sv from "./sv.json";
import nb from "./nb.json";
import da from "./da.json";
import fi from "./fi.json";
import vi from "./vi.json";
import id from "./id.json";
import tr from "./tr.json";

export const localeNames = {
  en: "English",
  pl: "Polski",
  es: "Español",
  fr: "Français",
  de: "Deutsch",
  "zh-CN": "简体中文",
  ja: "日本語",
  "pt-BR": "Português (Brasil)",
  ar: "العربية",
  hi: "हिन्दी",
  ko: "한국어",
  nl: "Nederlands",
  it: "Italiano",
  sv: "Svenska",
  nb: "Norsk bokmål",
  da: "Dansk",
  fi: "Suomi",
  vi: "Tiếng Việt",
  id: "Bahasa Indonesia",
  tr: "Türkçe",
} as const;
export type Locale = keyof typeof localeNames;
export const locales = Object.keys(localeNames) as Locale[];
export type Messages = { [K in keyof typeof en]: string };
const catalogs: Record<Locale, Messages> = {
  en: en,
  pl: pl,
  es: es,
  fr: fr,
  de: de,
  "zh-CN": zh_CN,
  ja: ja,
  "pt-BR": pt_BR,
  ar: ar,
  hi: hi,
  ko: ko,
  nl: nl,
  it: it,
  sv: sv,
  nb: nb,
  da: da,
  fi: fi,
  vi: vi,
  id: id,
  tr: tr,
};
export { en };
export const isLocale = (value: string | undefined): value is Locale =>
  !!value && Object.hasOwn(localeNames, value);
export const messages = (locale: Locale): Messages => catalogs[locale];
export const direction = (locale: Locale): "rtl" | "ltr" =>
  locale === "ar" ? "rtl" : "ltr";

/** Resolve Discord/browser locale variants to the supported regional defaults. */
export function resolveLocale(value: string | undefined): Locale {
  const normalized = value?.replaceAll("_", "-").toLowerCase() ?? "";
  const exact = locales.find((locale) => locale.toLowerCase() === normalized);
  if (exact) return exact;
  const language = normalized.split("-")[0];
  if (language === "zh") return "zh-CN";
  if (language === "pt") return "pt-BR";
  if (language === "no" || language === "nb") return "nb";
  return isLocale(language) ? language : "en";
}
