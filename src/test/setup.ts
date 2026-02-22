import "@testing-library/jest-dom/vitest";
import { afterEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";

afterEach(() => {
  cleanup();
});

vi.mock("react-i18next", () => {
  const i18n = {
    changeLanguage: vi.fn().mockResolvedValue(undefined),
    language: "ko",
  };

  return {
    useTranslation: () => ({
      t: (key: string, options?: Record<string, unknown>) => {
        if (options?.version && key === "settings.update.available") {
          return `${key}:${String(options.version)}`;
        }
        return key;
      },
      i18n,
    }),
    initReactI18next: {
      type: "3rdParty",
      init: () => undefined,
    },
  };
});

Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});
