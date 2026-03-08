import { test as base } from "@playwright/test";
import { SetupPage } from "../pages/SetupPage";
import { QueuePage } from "../pages/QueuePage";
import { SettingsPage } from "../pages/SettingsPage";

type Fixtures = {
  setupPage: SetupPage;
  queuePage: QueuePage;
  settingsPage: SettingsPage;
};

export const test = base.extend<Fixtures>({
  setupPage: async ({ page }, use) => {
    await use(new SetupPage(page));
  },
  queuePage: async ({ page }, use) => {
    await use(new QueuePage(page));
  },
  settingsPage: async ({ page }, use) => {
    await use(new SettingsPage(page));
  },
});

export { expect } from "@playwright/test";
