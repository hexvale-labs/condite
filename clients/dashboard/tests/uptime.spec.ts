import { test, expect } from "@playwright/test";
test.beforeEach(async ({ request }) => {
  await request.post("http://127.0.0.1:4322/__reset");
});
test("uptime advances with default mute, opt-in sound, refresh preservation and offline cleanup", async ({
  page,
  request,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.addInitScript(() => {
    Reflect.set(window, "__audioCreated", 0);
    const Original = window.AudioContext;
    window.AudioContext = class extends Original {
      constructor() {
        super();
        Reflect.set(
          window,
          "__audioCreated",
          Reflect.get(window, "__audioCreated") + 1,
        );
      }
    };
  });
  await page.goto("/en/service");
  const clock = page.locator(".condite-uptime");
  await expect(clock.locator(".condite-availability")).toHaveAttribute(
    "data-healthy",
    "true",
  );
  await expect(clock.locator(".condite-uptime-digits")).not.toContainText("PT");
  const start = await clock.locator(".condite-uptime-digits").textContent();
  await expect
    .poll(() => clock.locator(".condite-uptime-digits").textContent())
    .not.toBe(start);
  expect(await page.evaluate(() => Reflect.get(window, "__audioCreated"))).toBe(
    0,
  );
  await clock.getByRole("button", { name: "Enable clock ticking" }).click();
  await expect(
    clock.getByRole("button", { name: "Mute clock ticking" }),
  ).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(
    clock.getByRole("button", { name: "Mute clock ticking" }),
  ).toBeVisible();
  await request.post("http://127.0.0.1:4322/__failure", {
    data: { unavailable: true },
  });
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(clock.locator(".condite-availability")).toHaveAttribute(
    "data-healthy",
    "false",
  );
  await expect(clock.getByRole("button")).toBeDisabled();
  expect(errors).toEqual([]);
});
test("reduced motion disables pulse and hidden tabs reset audio to muted", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/en/service");
  const clock = page.locator(".condite-uptime");
  await expect(clock.getByRole("button")).toBeEnabled();
  expect(
    await clock
      .locator(".condite-availability")
      .evaluate((el) => getComputedStyle(el).animationName),
  ).toBe("none");
  await clock.getByRole("button").click();
  await expect(clock.getByRole("button")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await page.evaluate(() => {
    Object.defineProperty(document, "hidden", {
      configurable: true,
      value: true,
    });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await expect(clock.getByRole("button")).toHaveAttribute(
    "aria-pressed",
    "false",
  );
  await page.goto("/en/");
});
