import type { QueueSnapshot } from "./renderer/types";

declare global {
  interface Window {
    electronAPI?: {
      invoke<TResponse>(command: string, args?: Record<string, unknown>): Promise<TResponse>;
      onQueueUpdated(listener: (payload: QueueSnapshot) => void): () => void;
    };
  }
}

export {};
