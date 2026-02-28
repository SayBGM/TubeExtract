# TubeExtract Module Structure

## Module Organization

```text
src/
├── main.tsx                          # Application entry point
├── App.tsx                           # Root component and routing
├── domains/                          # Domain modules
│   ├── setup/                        # Setup domain
│   ├── queue/                        # Queue domain
│   └── settings/                     # Settings domain
├── shared/                           # Shared components and utilities
│   ├── components/
│   ├── hooks/
│   └── utils/
└── lib/
    └── desktopClient.ts              # Tauri IPC bridge
```

## Major Modules

### 1. Setup Domain

#### `SetupPage.tsx`
**Responsibility**: Main page component for URL analysis and format selection.

**Flow**
1. User enters URL
2. URL is validated in `SetupUrlForm`
3. `useSetupActions.analyzeUrl()` is called
4. Result is stored in `setupStore`
5. `SetupAnalysisResult` or `SetupAnalyzingState` is rendered

#### `useSetupActions.ts`
**Responsibility**: Setup domain business logic.

```typescript
analyzeUrl(url: string): Promise<AnalysisResult>
checkDuplicate(videoId: string): Promise<boolean>
selectFormat(format: Format): void
```

Dependencies: `desktopClient`, `setupStore`, React Query.

#### `setupStore.ts`
**Responsibility**: Setup domain state.

```typescript
currentUrl: string
analysisResult: AnalysisResult | null
isAnalyzing: boolean
selectedFormat: Format | null
```

### 2. Queue Domain

#### `QueuePage.tsx`
**Responsibility**: Main page for queue operations and live job status.

Dependencies: `queueActions`, `queueStore`, `useQueueEvents`, queue list components.

#### `queueActions.ts`
**Responsibility**: Core queue business logic and orchestration.

```typescript
enqueueJob(videoId: string, format: Format, options: DownloadOptions): Promise<JobId>
pauseJob(jobId: JobId): Promise<void>
resumeJob(jobId: JobId): Promise<void>
cancelJob(jobId: JobId): Promise<void>
clearTerminalJobs(): Promise<void>
```

Highlights:
- Job lifecycle management (`PENDING -> DOWNLOADING -> COMPLETED/FAILED`)
- Retry and fallback behavior
- Progress and throughput tracking

#### `queueStore.ts`
**Responsibility**: Queue domain state container.

```typescript
jobs: Job[]
activeJobs: Job[]
completedJobs: Job[]
```

#### `useQueueEvents.ts`
**Responsibility**: Subscribe to Tauri events and sync queue state.

Event types:
- `download-started`
- `download-progress`
- `download-completed`
- `download-failed`
- `download-paused`
- `download-resumed`

### 3. Settings Domain

#### `SettingsPage.tsx`
**Responsibility**: Main settings page.

#### `SettingsDiagnosticsSection`
**Responsibility**: Dependency diagnostic UI (yt-dlp/FFmpeg presence and version).

#### `SettingsUpdateSection`
**Responsibility**: Application version and update checks.

#### `SettingsDefaultsSection`
**Responsibility**: Default format and download path controls.

#### `settingsStore.ts`
**Responsibility**: User settings state.

```typescript
downloadDir: string
defaultFormat: Format
defaultOptions: DownloadOptions
language: string
```

### 4. Shared Modules

#### `Sidebar.tsx`
**Responsibility**: Application navigation (Setup, Queue, Settings).

#### `AppErrorBoundary.tsx`
**Responsibility**: Global runtime error capture and fallback UI.
