import js from "@eslint/js";
import globals from "globals";
export default [
  { ignores: ["**/dist/**", "**/target/**", "node_modules/**"] },
  {
    files: [
      "scripts/sdk/**/*.mjs",
      "scripts/sdk/**/*.cjs",
      "crates/condite-sdk-node/*.cjs",
      "crates/condite-sdk-wasm/*.js",
      "eslint.config.mjs",
    ],
    ...js.configs.recommended,
    languageOptions: { globals: { ...globals.node, ...globals.browser } },
  },
];
