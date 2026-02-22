import { create } from "zustand";
import type { AppSettings } from "../types";

const DEFAULT_SETTINGS: AppSettings = {
  downloadDir: "",
  maxRetries: 3,
  language: "ko",
};

interface SettingsStore {
  settings: AppSettings;
  setSettings: (value: AppSettings) => void;
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  settings: DEFAULT_SETTINGS,
  setSettings: (value) => set({ settings: value }),
}));
