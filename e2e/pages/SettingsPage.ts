import type { Page, Locator } from "@playwright/test";

export class SettingsPage {
  readonly page: Page;
  readonly saveButton: Locator;
  readonly languageSelect: Locator;
  readonly maxRetriesInput: Locator;
  readonly runDiagnosticsButton: Locator;
  readonly checkUpdateButton: Locator;
  readonly diagnosticsText: Locator;
  readonly updateMessageText: Locator;

  constructor(page: Page) {
    this.page = page;
    this.saveButton = page.getByTestId("settings-save-button");
    this.languageSelect = page.getByTestId("settings-language-select");
    this.maxRetriesInput = page.locator("#settings-max-retries");
    this.runDiagnosticsButton = page.getByRole("button", {
      name: /진단 실행/i,
    });
    this.checkUpdateButton = page.getByRole("button", {
      name: /업데이트 확인/i,
    });
    this.diagnosticsText = page
      .locator("p.text-sm.text-zinc-400")
      .filter({ hasText: /\S/ })
      .first();
    this.updateMessageText = page
      .locator("p.text-sm.text-zinc-400")
      .filter({ hasText: /\S/ })
      .last();
  }

  async goto() {
    await this.page.goto("/settings");
  }

  // react-hook-form registers uncontrolled inputs; pressSequentially simulates real key presses
  // which properly triggers React's synthetic event system and react-hook-form's onChange handler
  async fillMaxRetries(value: string) {
    await this.maxRetriesInput.click({ clickCount: 3 }); // triple click to select all
    await this.maxRetriesInput.pressSequentially(value, { delay: 30 });
  }
}
