import { QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createTestQueryClient } from "../../../test/utils/queryClient";
import { useSetupStore } from "../../store/setupStore";
import { useSetupActions } from "./useSetupActions";
import type { AnalysisResult } from "../../types";

const { analyzeUrlMock, checkDuplicateMock, enqueueJobMock } = vi.hoisted(() => ({
  analyzeUrlMock: vi.fn(),
  checkDuplicateMock: vi.fn(),
  enqueueJobMock: vi.fn(),
}));

vi.mock("../../lib/desktopClient", async () => {
  const actual = await vi.importActual<typeof import("../../lib/desktopClient")>(
    "../../lib/desktopClient",
  );

  return {
    ...actual,
    analyzeUrl: analyzeUrlMock,
    checkDuplicate: checkDuplicateMock,
    enqueueJob: enqueueJobMock,
  };
});

vi.mock("../../hooks/useOpenExternalUrl", () => ({
  useOpenExternalUrl: () => vi.fn().mockResolvedValue(undefined),
}));

const mockAnalysisResult: AnalysisResult = {
  sourceUrl: "https://youtu.be/abc1234",
  title: "Test Video",
  channel: "Channel",
  durationSec: 120,
  thumbnailUrl: "https://example.com/thumb.jpg",
  videoOptions: [{ id: "137+140", label: "1080p", ext: "mp4", type: "video" }],
  audioOptions: [{ id: "140", label: "128kbps", ext: "m4a", type: "audio" }],
};

describe("useSetupActions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSetupStore.setState({
      urlInput: "",
      isAnalyzing: false,
      analysisResult: undefined,
      selectedMode: "video",
      selectedQualityId: undefined,
      analyzeError: undefined,
    });

    checkDuplicateMock.mockResolvedValue({ isDuplicate: false });
    enqueueJobMock.mockResolvedValue({ jobId: "job-1" });
  });

  function wrapper({ children }: { children: ReactNode }) {
    const queryClient = createTestQueryClient();
    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
  }

  it("analyzes a normalized URL and updates setup store", async () => {
    analyzeUrlMock.mockResolvedValue(mockAnalysisResult);
    useSetupStore.setState({ urlInput: "  https://youtu.be/abc1234  " });

    const { result } = renderHook(() => useSetupActions(() => undefined), {
      wrapper,
    });

    await act(async () => {
      await result.current.onAnalyze();
    });

    expect(analyzeUrlMock).toHaveBeenCalledWith("https://youtu.be/abc1234");
    expect(useSetupStore.getState().analysisResult?.title).toBe("Test Video");
    expect(useSetupStore.getState().isAnalyzing).toBe(false);
  });

  it("enqueues a job after duplicate check", async () => {
    useSetupStore.setState({
      urlInput: "https://youtu.be/abc1234",
      analysisResult: mockAnalysisResult,
      selectedMode: "video",
      selectedQualityId: "137+140",
    });

    const { result } = renderHook(() => useSetupActions(() => undefined), {
      wrapper,
    });

    await act(async () => {
      await result.current.onEnqueue(false);
    });

    expect(checkDuplicateMock).toHaveBeenCalledTimes(1);
    expect(enqueueJobMock).toHaveBeenCalledWith({
      url: "https://youtu.be/abc1234",
      title: "Test Video",
      thumbnailUrl: "https://example.com/thumb.jpg",
      mode: "video",
      qualityId: "137+140",
      forceDuplicate: false,
    });
  });
});
