import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "./tests",
  testMatch: "**/*.spec.ts",
  workers: 1,
  use: { baseURL: "http://127.0.0.1:4321", headless: true },
  webServer: [
    { command: "bun tests/fixture.ts", port: 4322, reuseExistingServer: false },
    {
      command: process.env.DASHBOARD_DEV ? "bun run dev --host 127.0.0.1 --port 4321" : "node dist/server/entry.mjs",
      port: 4321,
      env: {
        ASTRO_DEV_BACKGROUND: "0",
        HOST: "127.0.0.1",
        PORT: "4321",
        CONDITE_API_URL: "http://127.0.0.1:4322",
      },
      reuseExistingServer: false,
    },
  ],
});
