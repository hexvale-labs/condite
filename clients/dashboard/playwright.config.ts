import { writeFileSync } from "node:fs";
import { defineConfig } from "@playwright/test";
writeFileSync("dashboard.test.toml", 'core_url = "http://127.0.0.1:4322"\norigins = "https://dashboard.example:8443,https://other.example:9443"\nhost = "127.0.0.1"\nport = 4321\n');
export default defineConfig({
  testDir: "./tests",
  testMatch: "**/*.spec.ts",
  // Tests reset one shared HTTP fixture; parallel workers would race.
  workers: 1,
  use: { baseURL: "http://127.0.0.1:4321", headless: true, storageState: {cookies:[{name:"condite_session",value:"fixture-session",domain:"127.0.0.1",path:"/",httpOnly:true,secure:false,sameSite:"Lax",expires:-1}],origins:[]} },
  webServer: [
    { command: "bun tests/fixture.ts", port: 4322, reuseExistingServer: false },
    {
      command: process.env.DASHBOARD_DEV ? "bun run dev --host 127.0.0.1 --port 4321 --condite-config dashboard.test.toml" : "node dist/server/entry.mjs --condite-config dashboard.test.toml",
      port: 4321,
      env: {
        ASTRO_DEV_BACKGROUND: "0",
        HOST: "127.0.0.1",
        DASHBOARD_ORIGINS: " https://dashboard.example:8443 , https://other.example:9443 ",
        PORT: "4321",
        CONDITE_API_URL: "http://127.0.0.1:4322",
      },
      reuseExistingServer: false,
    },
  ],
});
