import type { DependencyBootstrapStatus, QueueSnapshot } from "./renderer/types";

interface TauriEvent<TPayload> {
  payload: TPayload;
}

interface TauriApi {
  core: {
    invoke<TResponse>(command: string, args?: Record<string, unknown>): Promise<TResponse>;
  };
  event: {
    listen<TPayload>(
      eventName: string,
      callback: (event: TauriEvent<TPayload>) => void,
    ): Promise<() => void>;
  };
}

declare global {
  interface Window {
    __TAURI__?: TauriApi;
    __QUEUE_UPDATED__?: (payload: QueueSnapshot) => void;
    __DEPENDENCY_BOOTSTRAP_UPDATED__?: (payload: DependencyBootstrapStatus) => void;
  }
}

export {};
