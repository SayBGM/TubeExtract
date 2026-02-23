import type {
  AnalysisResult,
  AppSettings,
  DependencyBootstrapStatus,
  DiagnosticsResult,
  DownloadMode,
  DuplicateCheckResult,
  QueueItem,
  QueueSnapshot,
  StorageStats,
} from "../types";

export function isNativeDesktop() {
  return Boolean(window.__TAURI__?.core?.invoke);
}

function isTauriShell() {
  return navigator.userAgent.toLowerCase().includes("tauri");
}

function shouldUseMockMode() {
  if (isNativeDesktop()) return false;
  if (isTauriShell()) {
    throw new Error("Tauri bridge is unavailable. Restart the app.");
  }
  return true;
}

const mockQueue: QueueItem[] = [];
let mockSettings: AppSettings = {
  downloadDir: "",
  maxRetries: 3,
  language: "ko",
};

export const DESKTOP_CHANNEL = {
  ANALYZE_URL: "analyze_url",
  ENQUEUE_JOB: "enqueue_job",
  CHECK_DUPLICATE: "check_duplicate",
  PAUSE_JOB: "pause_job",
  RESUME_JOB: "resume_job",
  CANCEL_JOB: "cancel_job",
  CLEAR_TERMINAL_JOBS: "clear_terminal_jobs",
  DELETE_FILE: "delete_file",
  OPEN_FOLDER: "open_folder",
  OPEN_EXTERNAL_URL: "open_external_url",
  GET_QUEUE_SNAPSHOT: "get_queue_snapshot",
  GET_SETTINGS: "get_settings",
  PICK_DOWNLOAD_DIR: "pick_download_dir",
  SET_SETTINGS: "set_settings",
  RUN_DIAGNOSTICS: "run_diagnostics",
  CHECK_UPDATE: "check_update",
  GET_STORAGE_STATS: "get_storage_stats",
  GET_DEPENDENCY_BOOTSTRAP_STATUS: "get_dependency_bootstrap_status",
} as const;

type DesktopCommandName = (typeof DESKTOP_CHANNEL)[keyof typeof DESKTOP_CHANNEL];

function invokeCommand<TResponse>(
  command: DesktopCommandName,
  args?: Record<string, unknown>,
): Promise<TResponse> {
  const tauriApi = window.__TAURI__?.core;
  if (tauriApi) {
    return tauriApi.invoke<TResponse>(command, args);
  }

  throw new Error("Desktop API unavailable");
}

function subscribeToTauriEvent<TPayload>(
  eventName: string,
  listener: (payload: TPayload) => void,
): (() => void) | undefined {
  const tauriEventApi = window.__TAURI__?.event;
  if (!tauriEventApi) return undefined;

  let isDisposed = false;
  let unlisten: (() => void) | undefined;
  let retryTimer: number | undefined;

  const listenWithRetry = () => {
    if (isDisposed || unlisten) return;
    void tauriEventApi
      .listen<TPayload>(eventName, (event) => {
        listener(event.payload);
      })
      .then((cleanup) => {
        if (isDisposed) {
          cleanup();
          return;
        }
        unlisten = cleanup;
      })
      .catch(() => {
        // The runtime bridge can be late during initial boot; retry.
        if (isDisposed) return;
        retryTimer = window.setTimeout(listenWithRetry, 250);
      });
  };

  listenWithRetry();

  return () => {
    isDisposed = true;
    if (retryTimer) {
      clearTimeout(retryTimer);
    }
    unlisten?.();
  };
}

export async function analyzeUrl(url: string): Promise<AnalysisResult> {
  if (shouldUseMockMode()) {
    return {
      sourceUrl: url,
      title: "Mock Video Title",
      channel: "Mock Channel",
      durationSec: 120,
      thumbnailUrl: "https://placehold.co/640x360?text=Mock+Thumbnail",
      videoOptions: [
        { id: "137+140", label: "1080p", ext: "mp4", type: "video" },
        { id: "136+140", label: "720p", ext: "mp4", type: "video" },
      ],
      audioOptions: [{ id: "140", label: "128kbps", ext: "m4a", type: "audio" }],
    };
  }
  return invokeCommand(DESKTOP_CHANNEL.ANALYZE_URL, { url });
}

export async function enqueueJob(input: {
  url: string;
  title?: string;
  thumbnailUrl?: string;
  mode: DownloadMode;
  qualityId: string;
  forceDuplicate: boolean;
}): Promise<{ jobId: string }> {
  if (shouldUseMockMode()) {
    const id = crypto.randomUUID();
    mockQueue.unshift({
      id,
      title: input.title ?? input.url,
      thumbnailUrl: input.thumbnailUrl,
      url: input.url,
      mode: input.mode,
      qualityId: input.qualityId,
      status: "completed",
      progressPercent: 100,
      outputPath: "/tmp/mock.mp4",
      retryCount: 0,
      speedText: "2.3MiB/s",
      etaText: "00:00",
    });
    return { jobId: id };
  }
  return invokeCommand(DESKTOP_CHANNEL.ENQUEUE_JOB, { input });
}

export async function checkDuplicate(input: {
  url: string;
  mode: DownloadMode;
  qualityId: string;
}): Promise<DuplicateCheckResult> {
  if (shouldUseMockMode()) {
    const found = mockQueue.find(
      (item) => item.url === input.url && item.mode === input.mode && item.qualityId === input.qualityId,
    );
    return { isDuplicate: Boolean(found), existingOutputPath: found?.outputPath };
  }
  return invokeCommand(DESKTOP_CHANNEL.CHECK_DUPLICATE, { input });
}

export async function pauseJob(id: string): Promise<void> {
  if (shouldUseMockMode()) return;
  await invokeCommand<QueueSnapshot | void>(DESKTOP_CHANNEL.PAUSE_JOB, { id });
}

export async function resumeJob(id: string): Promise<void> {
  if (shouldUseMockMode()) return;
  await invokeCommand<QueueSnapshot | void>(DESKTOP_CHANNEL.RESUME_JOB, { id });
}

export async function cancelJob(id: string): Promise<void> {
  if (shouldUseMockMode()) {
    const target = mockQueue.find((item) => item.id === id);
    if (target) target.status = "canceled";
    return;
  }
  await invokeCommand<QueueSnapshot | void>(DESKTOP_CHANNEL.CANCEL_JOB, { id });
}

export async function clearTerminalJobs(): Promise<void> {
  if (shouldUseMockMode()) {
    for (let index = mockQueue.length - 1; index >= 0; index -= 1) {
      const status = mockQueue[index].status;
      if (status === "completed") {
        mockQueue.splice(index, 1);
      }
    }
    return;
  }
  await invokeCommand<QueueSnapshot | void>(DESKTOP_CHANNEL.CLEAR_TERMINAL_JOBS);
}

export async function deleteFile(path: string): Promise<void> {
  if (shouldUseMockMode()) {
    const index = mockQueue.findIndex((item) => item.outputPath === path);
    if (index >= 0) mockQueue.splice(index, 1);
    return;
  }
  await invokeCommand<QueueSnapshot | void>(DESKTOP_CHANNEL.DELETE_FILE, { path });
}

async function invokeQueueMutation(command: DesktopCommandName, args?: Record<string, unknown>) {
  if (shouldUseMockMode()) {
    return { items: mockQueue } as QueueSnapshot;
  }
  const response = await invokeCommand<QueueSnapshot | void>(command, args);
  if (response && typeof response === "object" && Array.isArray((response as QueueSnapshot).items)) {
    return response as QueueSnapshot;
  }
  return getQueueSnapshot();
}

export async function pauseJobAndGetSnapshot(id: string) {
  return invokeQueueMutation(DESKTOP_CHANNEL.PAUSE_JOB, { id });
}

export async function resumeJobAndGetSnapshot(id: string) {
  return invokeQueueMutation(DESKTOP_CHANNEL.RESUME_JOB, { id });
}

export async function cancelJobAndGetSnapshot(id: string) {
  return invokeQueueMutation(DESKTOP_CHANNEL.CANCEL_JOB, { id });
}

export async function clearTerminalJobsAndGetSnapshot() {
  return invokeQueueMutation(DESKTOP_CHANNEL.CLEAR_TERMINAL_JOBS);
}

export async function deleteFileAndGetSnapshot(path: string) {
  return invokeQueueMutation(DESKTOP_CHANNEL.DELETE_FILE, { path });
}

export async function openFolder(path: string): Promise<void> {
  if (shouldUseMockMode()) return;
  await invokeCommand(DESKTOP_CHANNEL.OPEN_FOLDER, { path });
}

export async function openExternalUrl(url: string): Promise<void> {
  if (shouldUseMockMode()) {
    window.open(url, "_blank", "noopener,noreferrer");
    return;
  }
  await invokeCommand(DESKTOP_CHANNEL.OPEN_EXTERNAL_URL, { url });
}

export async function getQueueSnapshot(): Promise<QueueSnapshot> {
  if (shouldUseMockMode()) return { items: mockQueue };
  return invokeCommand(DESKTOP_CHANNEL.GET_QUEUE_SNAPSHOT);
}

export async function getSettings(): Promise<AppSettings> {
  if (shouldUseMockMode()) return mockSettings;
  return invokeCommand(DESKTOP_CHANNEL.GET_SETTINGS);
}

export async function pickDownloadDir(): Promise<string | null> {
  if (shouldUseMockMode()) return null;
  return invokeCommand<string | null>(DESKTOP_CHANNEL.PICK_DOWNLOAD_DIR);
}

export async function setSettings(settings: AppSettings): Promise<void> {
  if (shouldUseMockMode()) {
    mockSettings = settings;
    return;
  }
  await invokeCommand(DESKTOP_CHANNEL.SET_SETTINGS, { settings });
}

export async function runDiagnostics(): Promise<DiagnosticsResult> {
  if (shouldUseMockMode()) {
    return {
      ytDlpAvailable: true,
      ffmpegAvailable: true,
      downloadPathWritable: true,
      message: "mock mode diagnostics: all green",
    };
  }
  return invokeCommand(DESKTOP_CHANNEL.RUN_DIAGNOSTICS);
}

export async function checkUpdate(): Promise<{
  hasUpdate: boolean;
  latestVersion?: string;
  url?: string;
}> {
  if (shouldUseMockMode()) return { hasUpdate: false };
  return invokeCommand(DESKTOP_CHANNEL.CHECK_UPDATE);
}

export async function getStorageStats(): Promise<StorageStats> {
  if (shouldUseMockMode()) {
    return {
      totalBytes: 100 * 1024 ** 3,
      availableBytes: 24.8 * 1024 ** 3,
      usedBytes: 75.2 * 1024 ** 3,
      usedPercent: 75.2,
      downloadDirBytes: 12.4 * 1024 ** 3,
    };
  }
  return invokeCommand(DESKTOP_CHANNEL.GET_STORAGE_STATS);
}

export async function getDependencyBootstrapStatus(): Promise<DependencyBootstrapStatus> {
  if (shouldUseMockMode()) {
    return {
      inProgress: false,
      phase: "ready",
      progressPercent: 100,
      errorMessage: undefined,
    };
  }
  return invokeCommand(DESKTOP_CHANNEL.GET_DEPENDENCY_BOOTSTRAP_STATUS);
}

export function onQueueUpdated(listener: (snapshot: QueueSnapshot) => void): (() => void) | undefined {
  if (shouldUseMockMode()) return undefined;
  return subscribeToTauriEvent<QueueSnapshot>("queue-updated", listener);
}

export function onDependencyBootstrapUpdated(
  listener: (status: DependencyBootstrapStatus) => void,
): (() => void) | undefined {
  if (shouldUseMockMode()) return undefined;
  return subscribeToTauriEvent<DependencyBootstrapStatus>("dependency-bootstrap-updated", listener);
}
