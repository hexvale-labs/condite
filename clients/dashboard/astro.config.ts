import { defineConfig, fontProviders } from "astro/config";
import node from "@astrojs/node";
import solid from "astro-solid-next";
import tailwind from "@tailwindcss/vite";
const origin = process.env.DASHBOARD_ORIGIN ? new URL(process.env.DASHBOARD_ORIGIN) : null;
export default defineConfig({
  security: { allowedDomains: [
    { hostname: "localhost" }, { hostname: "127.0.0.1" },
    ...(origin ? [{ hostname: origin.hostname, protocol: origin.protocol.slice(0, -1) as "http" | "https", port: origin.port }] : []),
  ] },
  output: "server",
  adapter: node({ mode: "standalone" }),
  integrations: [solid()],
  i18n: {
    defaultLocale: "en",
    locales: ["en", "es"],
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
  vite: { plugins: [tailwind()], ssr: { external: ["condite-sdk-node"] } },
});
