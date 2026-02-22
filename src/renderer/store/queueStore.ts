import { create } from "zustand";
import type { QueueItem } from "../types";

interface QueueStore {
  jobs: QueueItem[];
  applyQueueSnapshot: (jobs: QueueItem[]) => void;
}

export const useQueueStore = create<QueueStore>((set) => ({
  jobs: [],
  applyQueueSnapshot: (jobs) => set({ jobs }),
}));
