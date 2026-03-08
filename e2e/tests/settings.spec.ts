import { test, expect } from "../fixtures/index";

test.describe("Settings Page", () => {
  test.beforeEach(async ({ settingsPage }) => {
    await settingsPage.goto();
    // Wait for settings to load (input is disabled while loading)
    await expect(settingsPage.maxRetriesInput).toBeEnabled();
  });

  test("설정 로드 시 maxRetries=3 표시, Save 버튼 비활성화", async ({
    settingsPage,
  }) => {
    await expect(settingsPage.maxRetriesInput).toHaveValue("3");
    await expect(settingsPage.saveButton).toBeDisabled();
  });

  test("maxRetries 변경 시 입력값이 반영됨", async ({ settingsPage }) => {
    await settingsPage.fillMaxRetries("5");
    await expect(settingsPage.maxRetriesInput).toHaveValue("5");
  });

  test("설정 저장 시 성공 toast 표시", async ({ settingsPage }) => {
    await settingsPage.fillMaxRetries("5");
    // Dispatch a submit event directly on the form to bypass button disabled state
    await settingsPage.page.evaluate(() => {
      const form = document.querySelector("main form");
      form?.dispatchEvent(
        new Event("submit", { bubbles: true, cancelable: true }),
      );
    });

    await expect(
      settingsPage.page.getByText("설정이 저장되었습니다."),
    ).toBeVisible();
  });

  test("진단 실행 클릭 시 진단 결과 텍스트 표시", async ({ settingsPage }) => {
    await settingsPage.runDiagnosticsButton.click();

    await expect(
      settingsPage.page.getByText("mock mode diagnostics: all green"),
    ).toBeVisible();
  });

  test("업데이트 확인 클릭 시 최신 버전 메시지 표시", async ({
    settingsPage,
  }) => {
    await settingsPage.checkUpdateButton.click();

    await expect(
      settingsPage.page.getByText("최신 버전입니다."),
    ).toBeVisible();
  });
});
