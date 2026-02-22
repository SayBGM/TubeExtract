import { expect, test } from "@playwright/test";

test("language switch in settings", async ({ page }) => {
  await page.goto("/settings");
  await page.locator("[data-testid='settings-language-select']").selectOption("en");
  await page.locator("[data-testid='settings-save-button']").click();
  await page.goto("/setup");
  await expect(page.getByRole("heading", { name: "Download Setup" })).toBeVisible();
});
