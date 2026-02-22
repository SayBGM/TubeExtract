import { vi } from "vitest";
import type { QueueSnapshot } from "../../renderer/types";

type InvokeHandler = (
  command: string,
  args?: Record<string, unknown>,
) => unknown | Promise<unknown>;

interface CreateElectronIpcMockInput {
  handlers?: Partial<Record<string, InvokeHandler>>;
}

export function createElectronIpcMock(input: CreateElectronIpcMockInput = {}) {
  const { handlers = {} } = input;

  const invokeSpy = vi.fn(async (command: string, args?: Record<string, unknown>) => {
    const handler = handlers[command];
    if (!handler) {
      throw new Error(`Unhandled IPC command: ${command}`);
    }
    return handler(command, args);
  });

  const onQueueUpdatedSpy = vi.fn(
    (_listener: (payload: QueueSnapshot) => void) => () => undefined,
  );

  const electronIpcMock: NonNullable<Window["electronAPI"]> = {
    invoke: async <TResponse>(command: string, args?: Record<string, unknown>) =>
      invokeSpy(command, args) as Promise<TResponse>,
    onQueueUpdated: (listener: (payload: QueueSnapshot) => void) =>
      onQueueUpdatedSpy(listener),
  };

  return {
    ...electronIpcMock,
    invokeSpy,
    onQueueUpdatedSpy,
  };
}

export function installElectronIpcMock(mock = createElectronIpcMock()) {
  window.electronAPI = {
    invoke: mock.invoke,
    onQueueUpdated: mock.onQueueUpdated,
  };
  return mock;
}
