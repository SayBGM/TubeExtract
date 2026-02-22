import { expect, test } from "@playwright/test";

test("setup to queue mock flow works", async ({ page }) => {
  await page.goto("/setup");

  await page.locator("[data-testid='setup-url-input']").fill("https://www.youtube.com/watch?v=test");
  await page.locator("[data-testid='setup-analyze-button']").click();

  await expect(page.getByText("Mock Video Title")).toBeVisible();

  await page.locator("[data-testid='setup-download-now-button']").click();
  const forceSaveButton = page.getByRole("button", { name: /Save Anyway|그대로 저장/ });
  if (await forceSaveButton.isVisible().catch(() => false)) {
    await forceSaveButton.click();
  }

  await expect(page).toHaveURL(/\/queue$/, { timeout: 15_000 });
  await expect(page.getByText("Mock Video Title")).toBeVisible();
  await expect(page.getByText("Mock Video Title")).toBeVisible();
});
