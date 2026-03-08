import { test, expect } from "../fixtures/index";

const VALID_YT_URL = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";

test.describe("Setup Page", () => {
  test.beforeEach(async ({ setupPage }) => {
    await setupPage.goto();
  });

  test("빈 URL 상태에서 분석 버튼 비활성화", async ({ setupPage }) => {
    // Button is disabled when URL is empty — this is the intended UX behavior
    await expect(setupPage.analyzeButton).toBeDisabled();
  });

  test("유효하지 않은 URL 입력 시 에러 메시지 표시", async ({ setupPage }) => {
    await setupPage.urlInput.fill("https://example.com/notyoutube");
    // Button becomes enabled when there is text in the input
    await expect(setupPage.analyzeButton).toBeEnabled();
    await setupPage.analyzeButton.click();
    await expect(
      setupPage.page.getByText("유효한 유튜브 링크를 입력해 주세요."),
    ).toBeVisible();
  });

  test("유효한 YouTube URL 분석 시 Mock 결과 표시", async ({ setupPage }) => {
    await setupPage.analyze(VALID_YT_URL);
    await setupPage.waitForAnalysisResult();

    await expect(setupPage.page.getByText("Mock Video Title")).toBeVisible();
    // 1080p is shown in the SelectValue trigger (default selected quality)
    await expect(setupPage.page.getByText("1080p")).toBeVisible();
  });

  test("분석 후 다운로드 버튼 표시", async ({ setupPage }) => {
    await setupPage.urlInput.fill(VALID_YT_URL);
    await expect(setupPage.analyzeButton).toBeEnabled();

    await setupPage.analyzeButton.click();
    await setupPage.waitForAnalysisResult();
    await expect(setupPage.downloadButton).toBeVisible();
  });

  test("오디오 모드 전환 시 128kbps 옵션 표시", async ({ setupPage }) => {
    await setupPage.analyze(VALID_YT_URL);
    await setupPage.waitForAnalysisResult();

    await setupPage.selectAudioMode();
    // After switching to audio mode, the SelectValue shows "128kbps" (only audio option)
    await expect(setupPage.page.getByText("128kbps")).toBeVisible();
  });

  test("큐 추가 후 /queue 페이지로 이동", async ({ setupPage }) => {
    await setupPage.analyze(VALID_YT_URL);
    await setupPage.waitForAnalysisResult();

    await setupPage.downloadButton.click();
    await setupPage.page.waitForURL("**/queue");
    expect(setupPage.page.url()).toContain("/queue");
  });
});
