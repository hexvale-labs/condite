import { realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig, fontProviders } from "astro/config";
import node, { getAdapter } from "@astrojs/node";
import { dashboardConfig } from "./server/config";
import { allowedDomains } from "./server/origin";
import solid from "astro-solid-next";
import tailwind from "@tailwindcss/vite";
import { locales } from "../i18n";
export default defineConfig({
  security: { allowedDomains: allowedDomains(undefined) },
  output: "server",
  adapter: node({ mode: "standalone" }),
  integrations: [solid(), {
    name: "condite-runtime-origin",
    hooks: {
      "astro:config:setup": ({ command, updateConfig }) => {
        if (command === "dev") updateConfig({ security: { allowedDomains: allowedDomains(dashboardConfig().origins) } });
      },
      "astro:config:done": ({ setAdapter }) => setAdapter({
        ...getAdapter({ staticHeaders: false }),
        serverEntrypoint: new URL("./server/entrypoint.ts", import.meta.url),
      }),
    },
  }],
  i18n: {
    defaultLocale: "en",
    locales,
    routing: { prefixDefaultLocale: true },
  },
  fonts: [
    {
      name: "Public Sans",
      cssVariable: "--font-public-sans",
      provider: fontProviders.local(),
      options: {
        variants: [
          {
            src: [
              "./node_modules/@fontsource-variable/public-sans/files/public-sans-latin-wght-normal.woff2",
            ],
            weight: "100 900",
            style: "normal",
          },
        ],
      },
    },
  ],
  vite: { server: { fs: { allow: [fileURLToPath(new URL("../../", import.meta.url)), realpathSync(fileURLToPath(new URL("../../node_modules", import.meta.url)))] } }, plugins: [tailwind()], ssr: { external: ["condite-sdk-node"], noExternal: ["@condite/config"] } },
});
