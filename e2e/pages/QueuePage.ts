import type { Page, Locator } from "@playwright/test";

export class QueuePage {
  readonly page: Page;
  readonly clearButton: Locator;

  constructor(page: Page) {
    this.page = page;
    this.clearButton = page.getByRole("button", {
      name: /완료 리스트 비우기/i,
    });
  }

  async goto() {
    await this.page.goto("/queue");
  }
}
