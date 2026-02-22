import { create } from "zustand";

export interface ToastState {
  type: "success" | "error" | "info";
  message: string;
}

interface UIStore {
  toast?: ToastState;
  setToast: (value?: ToastState) => void;
}

export const useUIStore = create<UIStore>((set) => ({
  toast: undefined,
  setToast: (value) => set({ toast: value }),
}));
