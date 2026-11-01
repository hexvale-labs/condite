import { test, expect } from "@playwright/test";
import { locales, messages, direction } from "../../i18n";

test.beforeEach(async ({ request }) => {
  await request.post("http://127.0.0.1:4322/__reset");
});

test("every language renders files and service without JavaScript", async ({
  browser,
}) => {
  test.setTimeout(120_000);
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  for (const locale of locales) {
    for (const section of ["", "service"]) {
      await page.goto(`http://127.0.0.1:4321/${locale}/${section}`);
      await expect(page.locator("html")).toHaveAttribute("lang", locale);
      await expect(page.locator("html")).toHaveAttribute(
        "dir",
        direction(locale),
      );
      await expect(page.getByRole("heading", { level: 1 })).toHaveText(
        messages(locale)[section ? "service" : "files"],
      );
    }
  }
  await page.locator("summary").click();
  await page.getByRole("link", { name: "العربية", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("lang", "ar");
  await context.close();
});

test("Arabic hydrates and retains its language after file actions", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/ar/");
  await page
    .getByRole("button", { name: messages("ar").refresh, exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: messages("ar").refresh, exact: true }),
  ).toBeEnabled();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  expect(errors).toEqual([]);
});

test("localized sharing dialogs work on mobile and preserve RTL keyboard focus", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.emulateMedia({ reducedMotion: "reduce" });
  for (const locale of ["es", "ar", "de", "ja"] as const) {
    const t = messages(locale);
    await page.goto(`/${locale}/?view=grid`);
    const card = page.getByRole("listitem").filter({ hasText: "notes.txt" });
    const share = card.getByRole("button", { name: t.share, exact: true });
    await share.click();
    const dialog = page.getByRole("dialog");
    await expect(dialog.getByRole("heading")).toHaveText(t.share);
    await dialog.getByRole("combobox").selectOption("never");
    await dialog
      .getByRole("button", { name: t.createShare, exact: true })
      .click();
    await expect(dialog.getByRole("textbox")).toHaveValue(/\/public\//);
    await page.keyboard.press("Escape");
    await expect(dialog).not.toBeVisible();
    await expect(share).toBeFocused();
    await card.getByRole("button", { name: t.retention, exact: true }).click();
    await expect(dialog.getByRole("heading")).toHaveText(t.retention);
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await page.keyboard.press("Escape");
    await expect(dialog).not.toBeVisible();
  }
});
