import { create } from "zustand";
import type { AnalysisResult } from "../types";

interface SetupStore {
  urlInput: string;
  isAnalyzing: boolean;
  analysisResult?: AnalysisResult;
  selectedMode: "video" | "audio";
  selectedQualityId?: string;
  analyzeError?: string;
  setUrlInput: (value: string) => void;
  setAnalyzing: (value: boolean) => void;
  setAnalysisResult: (value?: AnalysisResult) => void;
  setSelectedMode: (value: "video" | "audio") => void;
  setSelectedQualityId: (value?: string) => void;
  setAnalyzeError: (value?: string) => void;
}

export const useSetupStore = create<SetupStore>((set) => ({
  urlInput: "",
  isAnalyzing: false,
  selectedMode: "video",
  setUrlInput: (value) => set({ urlInput: value }),
  setAnalyzing: (value) => set({ isAnalyzing: value }),
  setAnalysisResult: (value) =>
    set((state) => {
      const preferredOptions =
        state.selectedMode === "audio" ? value?.audioOptions : value?.videoOptions;
      const fallbackOptions =
        state.selectedMode === "audio" ? value?.videoOptions : value?.audioOptions;
      const defaultQualityId = preferredOptions?.[0]?.id ?? fallbackOptions?.[0]?.id;
      return { analysisResult: value, selectedQualityId: defaultQualityId };
    }),
  setSelectedMode: (value) =>
    set((state) => {
      const qualityList =
        value === "video"
          ? state.analysisResult?.videoOptions
          : state.analysisResult?.audioOptions;
      return {
        selectedMode: value,
        selectedQualityId: qualityList?.[0]?.id,
      };
    }),
  setSelectedQualityId: (value) => set({ selectedQualityId: value }),
  setAnalyzeError: (value) => set({ analyzeError: value }),
}));
