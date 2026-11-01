# Client translations

MIT-licensed, dependency-free messages shared by the dashboard and Condite's
Vencord plugin (including Vesktop). Each locale has a complete JSON dictionary;
`en.json` defines the keys. Add each new key to every dictionary. Type checking
and `bun test clients/i18n/catalog.test.ts` enforce parity.

`pending` means the upload record is waiting for storage confirmation;
`confirmed` means confirmation succeeded, not that the file is publicly shared.
Keep temporary-link expiry and irreversible-deletion warnings explicit.

`index.ts` owns supported locales, native names, regional aliases and direction.
Chinese uses Simplified Chinese, Portuguese uses Brazilian Portuguese, and
Norwegian uses Bokmål. Unsupported languages fall back to English.

The dashboard uses explicit locale URLs and a JavaScript-free language menu.
Vencord follows Discord's reactive `LocaleStore`, falling back to the browser
language while the store is unavailable. Sync copies these resources into the
managed Vencord checkout; rebuild after editing. No online translator is used.

Technical plugin metadata and raw SDK/server errors may remain in English.
Check translations in context; key parity cannot prove linguistic quality.

`errors.ts` formats SDK errors for both clients. The standalone desktop plugin
ships an MIT copy in `../vencord/condite.desktop/errors.ts`; its parity test
requires updating both together. Raw details retain the original response text.
