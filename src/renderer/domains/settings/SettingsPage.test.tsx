import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders } from "../../../test/utils/renderWithProviders";

const initialSettings = {
  downloadDir: "/downloads",
  maxRetries: 3,
  language: "ko" as const,
};

const {
  setQueryDataMock,
  getSettingsMock,
  setSettingsMock,
  pickDownloadDirMock,
  runDiagnosticsMock,
  checkUpdateMock,
  openExternalUrlMock,
} = vi.hoisted(() => ({
  setQueryDataMock: vi.fn(),
  getSettingsMock: vi.fn(),
  setSettingsMock: vi.fn(),
  pickDownloadDirMock: vi.fn(),
  runDiagnosticsMock: vi.fn(),
  checkUpdateMock: vi.fn(),
  openExternalUrlMock: vi.fn(),
}));

vi.mock("@tanstack/react-query", async () => {
  const actual =
    await vi.importActual<typeof import("@tanstack/react-query")>(
      "@tanstack/react-query",
    );

  return {
    ...actual,
    useQueryClient: () => ({
      setQueryData: setQueryDataMock,
    }),
    useQuery: () => ({
      data: initialSettings,
      isPending: false,
      error: null,
    }),
    useMutation: ({ mutationFn, onSuccess, onError }: any) => ({
      isPending: false,
      mutate: async (variables?: unknown) => {
        try {
          const result = await mutationFn(variables);
          await onSuccess?.(result, variables);
          return result;
        } catch (error) {
          onError?.(error);
          throw error;
        }
      },
      mutateAsync: async (variables?: unknown) => {
        try {
          const result = await mutationFn(variables);
          await onSuccess?.(result, variables);
          return result;
        } catch (error) {
          onError?.(error);
          throw error;
        }
      },
    }),
  };
});

vi.mock("../../lib/electronClient", async () => {
  const actual = await vi.importActual<typeof import("../../lib/electronClient")>(
    "../../lib/electronClient",
  );

  return {
    ...actual,
    getSettings: getSettingsMock,
    setSettings: setSettingsMock,
    pickDownloadDir: pickDownloadDirMock,
    runDiagnostics: runDiagnosticsMock,
    checkUpdate: checkUpdateMock,
    openExternalUrl: openExternalUrlMock,
  };
});

describe("SettingsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    getSettingsMock.mockResolvedValue(initialSettings);
    setSettingsMock.mockResolvedValue(undefined);
    pickDownloadDirMock.mockResolvedValue(null);
    runDiagnosticsMock.mockResolvedValue({ message: "ok" });
    checkUpdateMock.mockResolvedValue({ hasUpdate: false });
    openExternalUrlMock.mockResolvedValue(undefined);
  });

  it("keeps save disabled when clean", async () => {
    const { SettingsPage } = await import("./SettingsPage");
    renderWithProviders(<SettingsPage />);

    const saveButton = screen.getByTestId("settings-save-button");
    expect(saveButton).toBeDisabled();
  });

  it("submits settings and keeps save disabled after success", async () => {
    const { SettingsPage } = await import("./SettingsPage");
    renderWithProviders(<SettingsPage />);

    const saveButton = screen.getByTestId("settings-save-button");
    fireEvent.submit(saveButton.closest("form") as HTMLFormElement);

    await waitFor(() => {
      expect(setSettingsMock).toHaveBeenCalledWith({
        downloadDir: "/downloads",
        maxRetries: 3,
        language: "ko",
      });
    });

    await waitFor(() => {
      expect(setQueryDataMock).toHaveBeenCalled();
      expect(saveButton).toBeDisabled();
    });
  });
});
