export type DownloadMode = "video" | "audio";

export type JobStatus =
  | "queued"
  | "analyzing"
  | "downloading"
  | "paused"
  | "completed"
  | "failed"
  | "canceled";

export interface QualityOption {
  id: string;
  label: string;
  ext: string;
  type: DownloadMode;
}

export interface AnalysisResult {
  sourceUrl: string;
  title: string;
  channel: string;
  durationSec: number;
  thumbnailUrl: string;
  videoOptions: QualityOption[];
  audioOptions: QualityOption[];
}

export interface QueueItem {
  id: string;
  title: string;
  thumbnailUrl?: string;
  url: string;
  mode: DownloadMode;
  qualityId: string;
  status: JobStatus;
  progressPercent: number;
  speedText?: string;
  etaText?: string;
  outputPath?: string;
  errorMessage?: string;
  retryCount: number;
  downloadLog?: string[];
}

export interface QueueSnapshot {
  items: QueueItem[];
}

export interface DiagnosticsResult {
  ytDlpAvailable: boolean;
  ffmpegAvailable: boolean;
  downloadPathWritable: boolean;
  message: string;
}

export interface AppSettings {
  downloadDir: string;
  maxRetries: number;
  language: "ko" | "en";
}

export interface DuplicateCheckResult {
  isDuplicate: boolean;
  existingOutputPath?: string;
}

export interface StorageStats {
  totalBytes: number;
  availableBytes: number;
  usedBytes: number;
  usedPercent: number;
  downloadDirBytes: number;
}
