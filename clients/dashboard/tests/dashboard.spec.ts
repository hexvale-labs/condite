import { test, expect } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
test.beforeEach(async ({ request }) => {
  await request.post("http://127.0.0.1:4322/__reset");
});

test("server renders files and service information with JavaScript disabled", async ({
  browser,
}) => {
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  await page.goto("http://127.0.0.1:4321/en/");
  await expect(
    page
      .getByRole("cell")
      .filter({ has: page.getByText("notes.txt", { exact: true }) }),
  ).toBeVisible();
  await expect(page.getByText("16 EiB", { exact: true })).toBeVisible();
  await page.goto("http://127.0.0.1:4321/en/service");
  await expect(page.getByText("fixture", { exact: true })).toBeVisible();
  await context.close();
});

test("hydrates without replacing file DOM and preserves grid through reload and locale switch", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (message) => {
    if (
      ["error", "warning"].includes(message.type()) &&
      /hydrat/i.test(message.text())
    )
      errors.push(message.text());
  });
  let releaseScripts!: () => void;
  const scriptsReady = new Promise<void>((resolve) => {
    releaseScripts = resolve;
  });
  await page.route("**/*", async (route) => {
    if (route.request().resourceType() === "script") await scriptsReady;
    await route.continue();
  });
  await page.addInitScript(() => {
    (window as any).__shifts = 0;
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries() as any)
        if (!entry.hadRecentInput) (window as any).__shifts += entry.value;
    }).observe({ type: "layout-shift", buffered: true });
  });
  await page.goto("/en/", { waitUntil: "commit" });
  const file = page.getByText("notes.txt", { exact: true });
  await expect(file).toBeVisible();
  const row = await file.elementHandle();
  await expect(
    page.getByRole("button", { name: "Grid view", exact: true }),
  ).toBeDisabled();
  releaseScripts();
  await expect(
    page.getByRole("button", { name: "Grid view", exact: true }),
  ).toBeEnabled();
  expect(await row!.evaluate((el) => el.isConnected)).toBe(true);
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Refresh", exact: true }),
  ).toBeEnabled();
  expect(await row!.evaluate((el) => el.isConnected)).toBe(true);
  await page.getByRole("button", { name: "Grid view", exact: true }).click();
  await expect(
    page.getByRole("list", { name: "Files", exact: true }),
  ).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole("list", { name: "Files", exact: true }),
  ).toBeVisible();
  await page.locator("summary").click();
  await page.getByRole("link", { name: "Español" }).click();
  await expect(page.locator("html")).toHaveAttribute("lang", "es");
  await expect(
    page.getByRole("list", { name: "Archivos", exact: true }),
  ).toBeVisible();
  expect(errors).toEqual([]);
  expect(await page.evaluate(() => (window as any).__shifts)).toBeLessThan(0.1);
});

test("confirms, creates a temporary link, and deletes with keyboard focus restored", async ({
  page,
}) => {
  await page.goto("/en/");
  await page
    .getByRole("button", { name: "Confirm upload", exact: true })
    .click();
  await expect(page.getByRole("status")).toContainText("Upload confirmed.");
  await page
    .getByRole("row")
    .filter({ hasText: "notes.txt" })
    .getByRole("button", { name: "Download", exact: true })
    .click();
  await expect(
    page.getByRole("link", { name: "Open file", exact: true }),
  ).toHaveAttribute("href", /\/download\//);
  await page
    .getByRole("row")
    .filter({ hasText: "pending.txt" })
    .getByRole("button", { name: "Delete", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).not.toBeVisible();
  const remove = page
    .getByRole("row")
    .filter({ hasText: "pending.txt" })
    .getByRole("button", { name: "Delete", exact: true });
  await expect(remove).toBeFocused();
  await remove.click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Delete", exact: true })
    .click();
  await expect(page.getByText("pending.txt", { exact: true })).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Upload files", exact: true }),
  ).toBeFocused();
});

test("uploads via WASM and direct PUT, retaining recoverable failures", async ({
  page,
  request,
}) => {
  await page.goto("/en/");
  await expect(
    page.getByRole("button", { name: "Upload files", exact: true }),
  ).toBeEnabled();
  await page.locator("input[type=file]").setInputFiles({
    name: "hello.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("hello"),
  });
  await expect(
    page.getByRole("region", { name: "Upload activity" }),
  ).toContainText("Complete");
  await expect(page.getByRole("link", { name: "Next page" })).toBeVisible();
  const calls: string[] = await (
    await request.get("http://127.0.0.1:4322/__calls")
  ).json();
  expect(calls.some((c) => c.startsWith("PUT /storage/"))).toBe(true);
  await request.post("http://127.0.0.1:4322/__failure", {
    data: { confirm: true },
  });
  await page.locator("input[type=file]").setInputFiles({
    name: "failed.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("fail"),
  });
  await expect(
    page.getByRole("region", { name: "Upload activity" }),
  ).toContainText("Pending upload ID");
  await expect(
    page.getByRole("region", { name: "Upload activity" }),
  ).toContainText("Failed");
});

test("both locales and file views have no serious accessibility violations or mobile overflow", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  for (const route of [
    "/en/",
    "/en/?view=grid",
    "/es/",
    "/es/?view=grid",
    "/en/service",
    "/es/service",
  ]) {
    await page.goto(route);
    await expect(
      page.getByRole("button", { name: /^(Refresh|Actualizar)$/ }),
    ).toBeEnabled();
    await page
      .locator(".file-card, .file-row")
      .evaluateAll((elements) =>
        Promise.all(
          elements.flatMap((element) =>
            element.getAnimations().map((animation) => animation.finished),
          ),
        ),
      );
    const result = await new AxeBuilder({ page }).analyze();
    expect(result.violations).toEqual([]);
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
  }
});

test("failed deletion stays visible inside the dialog and can be retried", async ({
  page,
  request,
}) => {
  await page.goto("/en/?view=grid");
  const card = page.getByRole("listitem").filter({ hasText: "pending.txt" });
  await card.getByRole("button", { name: "Delete", exact: true }).click();
  await request.post("http://127.0.0.1:4322/__failure", {
    data: { delete: true },
  });
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Delete", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toContainText("The request failed.");
  await expect(card).toBeVisible();
  await request.post("http://127.0.0.1:4322/__failure", { data: {} });
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Delete", exact: true })
    .click();
  await expect(card).toHaveCount(0);
});

test("loads WASM only on interaction, keeps stale files on failure, and recovers", async ({
  page,
  request,
}) => {
  const wasm: string[] = [];
  page.on("request", (r) => {
    if (r.url().endsWith(".wasm")) wasm.push(r.url());
  });
  await page.goto("/en/");
  const refresh = page.getByRole("button", { name: "Refresh", exact: true });
  await expect(refresh).toBeEnabled();
  expect(wasm).toHaveLength(0);
  await request.post("http://127.0.0.1:4322/__failure", {
    data: { unavailable: true },
  });
  await refresh.click();
  await expect(page.getByRole("status")).toContainText("Could not load files.");
  await expect(page.getByText("notes.txt", { exact: true })).toBeVisible();
  expect(wasm).toHaveLength(1);
  await request.post("http://127.0.0.1:4322/__failure", { data: {} });
  await page.getByRole("button", { name: "Try again", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Try again", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.locator('[title="18446744073709551615 bytes"]'),
  ).toBeVisible();
});

test("pending confirmation and failed PUT are recoverable; cursor and grid actions work", async ({
  page,
  request,
}) => {
  await page.goto("/en/?view=grid");
  await request.post("http://127.0.0.1:4322/__failure", {
    data: { confirm: true },
  });
  await page
    .getByRole("button", { name: "Confirm upload", exact: true })
    .click();
  await expect(page.getByRole("status")).toContainText("The request failed.");
  await expect(
    page.getByRole("button", { name: "Confirm upload", exact: true }),
  ).toBeVisible();
  await request.post("http://127.0.0.1:4322/__failure", {
    data: { put: true },
  });
  await page.locator("input[type=file]").setInputFiles({
    name: "retry.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("fail"),
  });
  await expect(
    page.getByRole("region", { name: "Upload activity" }),
  ).toContainText("Pending upload ID");
  await page.getByRole("link", { name: "Next page", exact: true }).click();
  await expect(page.getByRole("list")).toContainText("retry.txt");
  await expect(
    page.getByRole("button", { name: "Confirm upload", exact: true }),
  ).toBeVisible();
  await page.getByRole("link", { name: "First page", exact: true }).click();
  await expect(page.getByRole("list")).toContainText("notes.txt");
});

test("localized service refresh reports outages, and invalid routes fail", async ({
  page,
  request,
}) => {
  await page.goto("/es/service");
  await expect(page.getByText("fixture", { exact: true })).toBeVisible();
  await request.post("http://127.0.0.1:4322/__failure", {
    data: { unavailable: true },
  });
  await page.getByRole("button", { name: "Actualizar", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("No se pudo conectar");
  expect((await request.get("/zz/")).status()).toBe(404);
  expect((await request.get("/en/?after=invalid")).status()).toBe(400);
  expect(
    (
      await request.delete("/api/v1/uploads/id", {
        headers: { Origin: "https://other.example" },
      })
    ).status(),
  ).toBe(403);
  expect((await request.get("/api/anything")).status()).toBe(404);
});

test("stalled core still renders localized SSR retry states", async ({
  browser,
  request,
}) => {
  await request.post("http://127.0.0.1:4322/__failure", {
    data: { stalled: true },
  });
  const context = await browser.newContext({ javaScriptEnabled: false });
  try {
    const page = await context.newPage();
    await page.goto("/en/", { timeout: 8000 });
    await expect(
      page.getByRole("button", { name: "Try again", exact: true }),
    ).toBeVisible();
    await page.goto("/es/service", { timeout: 8000 });
    await expect(page.getByRole("status")).not.toBeEmpty();
    await expect(
      page.getByRole("heading", { name: "Servicio", exact: true }),
    ).toBeVisible();
  } finally {
    await context.close();
  }
});

test("shares stay independent, revocation can retry, and retention supports Never", async ({
  page,
  request,
}) => {
  await page.goto("/en/");
  const row = page.getByRole("row").filter({ hasText: "notes.txt" });
  for (let i = 0; i < 3; i++) {
    await row.getByRole("button", { name: "Share", exact: true }).click();
    const dialog = page.getByRole("dialog");
    await dialog
      .getByRole("button", { name: "Create share link", exact: true })
      .click();
    await expect(dialog.getByRole("textbox")).toHaveValue(/\/public\/share-/);
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
  }
  await row.getByRole("button", { name: "Manage links", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("listitem")).toHaveCount(2);
  await dialog.getByRole("button", { name: "Next page", exact: true }).click();
  await expect(dialog.getByRole("listitem")).toHaveCount(1);
  await dialog.getByRole("button", { name: "First page", exact: true }).click();
  await request.post("http://127.0.0.1:4322/__failure", {
    data: { revoke: true },
  });
  await dialog
    .getByRole("button", { name: "Revoke", exact: true })
    .first()
    .click();
  await expect(dialog.getByRole("alert")).toContainText("The request failed.");
  await request.post("http://127.0.0.1:4322/__failure", { data: {} });
  await dialog
    .getByRole("button", { name: "Revoke", exact: true })
    .first()
    .click();
  await expect(dialog.getByRole("listitem").first()).toContainText("Revoked");
  await expect(dialog.getByRole("listitem").nth(1)).toContainText("Active");
  await dialog.getByRole("button", { name: "Close", exact: true }).click();
  await row.getByRole("button", { name: "Expiry", exact: true }).click();
  await dialog.getByRole("combobox").selectOption("never");
  await dialog
    .getByRole("button", { name: "Save expiry", exact: true })
    .click();
  await expect(dialog.getByRole("status")).toContainText("Expiry updated.");
  await dialog.getByRole("button", { name: "Close", exact: true }).click();
  await expect(row).toContainText("Never");
  const calls: string[] = await (
    await request.get("http://127.0.0.1:4322/__calls")
  ).json();
  expect(calls.some((call) => call.startsWith("PATCH /v1/uploads/"))).toBe(
    true,
  );
});

test("private upload can choose Never before PUT without publishing", async ({
  page,
  request,
}) => {
  await page.goto("/en/");
  await page
    .getByRole("button", { name: "Upload expiry", exact: true })
    .click();
  await page.getByRole("combobox").first().selectOption("never");
  await page.locator("input[type=file]").setInputFiles({
    name: "private.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("private"),
  });
  await expect(
    page.getByRole("region", { name: "Upload activity" }),
  ).toContainText("Complete");
  const calls: string[] = await (
    await request.get("http://127.0.0.1:4322/__calls")
  ).json();
  expect(calls.some((call) => /POST .*\/shares$/.test(call))).toBe(false);
  await page.getByRole("link", { name: "Next page", exact: true }).click();
  const row = page.getByRole("row").filter({ hasText: "private.txt" });
  await expect(row).toContainText("Private");
  await expect(row).toContainText("Never");
});

test("expiry intervals adjust the existing deadline and past dates require deletion confirmation", async ({
  page,
}) => {
  await page.goto("/en/?view=grid");
  const card = page.getByRole("listitem").filter({ hasText: "notes.txt" });
  await card.getByRole("button", { name: "Expiry", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("combobox").selectOption("interval");
  await dialog.getByRole("combobox").nth(1).selectOption("subtract");
  await dialog.getByRole("spinbutton").fill("7");
  const updated = page.waitForResponse(
    (response) => response.request().method() === "PATCH",
  );
  await dialog
    .getByRole("button", { name: "Save expiry", exact: true })
    .click();
  expect((await (await updated).json()).expires_at).toBe(
    "2098-12-25T00:00:00.000Z",
  );
  await dialog.getByRole("combobox").first().selectOption("date");
  await dialog.locator('input[type="datetime-local"]').fill("2000-01-01T00:00");
  await dialog.getByRole("button", { name: "Delete", exact: true }).click();
  await expect(dialog.getByRole("alert")).toContainText(
    "This date has passed.",
  );
  await expect(card).toBeVisible();
  await dialog.getByRole("checkbox").check();
  await dialog.getByRole("button", { name: "Delete", exact: true }).click();
  await expect(dialog).not.toBeVisible();
  await expect(card).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Upload files", exact: true }),
  ).toBeFocused();
});

test("pending revocation shows cleanup status and never exposes an unpublished URL", async ({
  page,
  request,
}) => {
  await page.goto("/en/");
  const row = page.getByRole("row").filter({ hasText: "notes.txt" });
  await row.getByRole("button", { name: "Share", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await dialog
    .getByRole("button", { name: "Create share link", exact: true })
    .click();
  await expect(dialog.getByRole("textbox")).toHaveValue(/\/public\//);
  await dialog.getByRole("button", { name: "Close", exact: true }).click();
  await row.getByRole("button", { name: "Manage links", exact: true }).click();
  await expect(dialog.getByRole("listitem")).toHaveCount(1);
  await request.post("http://127.0.0.1:4322/__failure", {
    data: { revokePending: true },
  });
  await dialog.getByRole("button", { name: "Revoke", exact: true }).click();
  await expect(dialog.getByRole("status")).toHaveText("Deletion pending");
  await expect(dialog.getByRole("listitem")).toContainText("Revoking");
  await expect(
    dialog.getByRole("button", { name: "Copy link", exact: true }),
  ).toBeDisabled();
  await expect(dialog.getByRole("textbox")).toHaveValue("");
});
