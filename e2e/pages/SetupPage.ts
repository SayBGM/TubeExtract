import type { Page, Locator } from "@playwright/test";

export class SetupPage {
  readonly page: Page;
  readonly urlInput: Locator;
  readonly analyzeButton: Locator;
  readonly downloadButton: Locator;

  constructor(page: Page) {
    this.page = page;
    this.urlInput = page.getByTestId("setup-url-input");
    this.analyzeButton = page.getByTestId("setup-analyze-button");
    this.downloadButton = page.getByTestId("setup-download-now-button");
  }

  async goto() {
    await this.page.goto("/setup");
  }

  async analyze(url: string) {
    await this.urlInput.fill(url);
    await this.analyzeButton.click();
  }

  async waitForAnalysisResult() {
    await this.page.waitForSelector('[data-testid="setup-download-now-button"]');
  }

  async selectAudioMode() {
    await this.page.getByRole("button", { name: /오디오/i }).click();
  }
}
