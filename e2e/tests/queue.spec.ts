import { test, expect } from "../fixtures/index";

const VALID_YT_URL = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";

test.describe("Queue Page", () => {
  test("빈 큐 상태에서 다운로드 이력 없음 메시지 및 Clear 버튼 비활성화", async ({
    queuePage,
  }) => {
    await queuePage.goto();

    await expect(
      queuePage.page.getByText("다운로드 이력이 없습니다."),
    ).toBeVisible();
    await expect(queuePage.clearButton).toBeDisabled();
  });

  test("Setup에서 enqueue 후 큐 페이지에 아이템 표시", async ({
    page,
    setupPage,
    queuePage,
  }) => {
    // Setup: analyze and enqueue
    await setupPage.goto();
    await setupPage.analyze(VALID_YT_URL);
    await setupPage.waitForAnalysisResult();
    await setupPage.downloadButton.click();

    // Wait for navigation to /queue
    await page.waitForURL("**/queue");

    // Queue page should show the completed item (mockQueue polling syncs within 300ms)
    await expect(
      queuePage.page.getByText("Mock Video Title"),
    ).toBeVisible();
    await expect(queuePage.clearButton).toBeEnabled();
  });

  test("Clear Completed 클릭 시 확인 모달 표시 및 상호작용 가능", async ({
    page,
    setupPage,
    queuePage,
  }) => {
    // Enqueue an item first
    await setupPage.goto();
    await setupPage.analyze(VALID_YT_URL);
    await setupPage.waitForAnalysisResult();
    await setupPage.downloadButton.click();
    await page.waitForURL("**/queue");

    // Wait for item to appear
    await expect(queuePage.page.getByText("Mock Video Title")).toBeVisible();

    // Click clear completed
    await queuePage.clearButton.click();

    // Verify the confirm dialog appears with correct description text
    await expect(
      page.getByText("완료 항목을 목록에서 지울까요?"),
    ).toBeVisible();

    // Click cancel to close the dialog without side effects
    await page.getByRole("button", { name: /취소/i }).click();

    // Dialog should be dismissed
    await expect(
      page.getByText("완료 항목을 목록에서 지울까요?"),
    ).not.toBeVisible();
  });
});
