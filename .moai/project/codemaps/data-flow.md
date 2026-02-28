# TubeExtract Data Flow

## End-to-End Flow Summary

TubeExtract uses two explicit flow directions:
- **UI -> State -> Backend Commands** for user-triggered actions
- **Backend Events -> State Updates -> UI Re-render** for real-time feedback

## Primary Flow Paths

### 1. Download Workflow (Core Path)

#### Phase 1: URL Analysis (Setup Domain)

```text
SetupPage (user input)
    -> SetupUrlForm.onChange
    -> setupStore.setCurrentUrl()
    -> useSetupActions.analyzeUrl()
    -> setupStore.setIsAnalyzing(true)
    -> desktopClient.analyzeUrl(url)
    -> Tauri invoke("analyze_url")
    -> [Rust] yt-dlp --dump-json URL
    -> structured JSON response
    -> setupStore.setAnalysisResult(result)
    -> setupStore.setIsAnalyzing(false)
    -> SetupAnalysisResult render
```

State transitions:
1. `currentUrl`: `""` -> URL value
2. `isAnalyzing`: `false` -> `true`
3. `analysisResult`: `null` -> metadata object
4. `isAnalyzing`: `true` -> `false`

#### Phase 2: Format Selection (Setup Domain)

```text
SetupAnalysisResult (format select)
    -> onFormatChange(format)
    -> setupStore.setSelectedFormat(format)
    -> user clicks Download
    -> queueActions.enqueueJob()
```

State transition:
- `selectedFormat`: `null` -> selected format object

#### Phase 3: Queueing and Download Execution (Queue Domain)

```text
queueActions.enqueueJob(params)
    -> desktopClient.enqueueJob(...)
    -> Tauri invoke("enqueue_job")
    -> [Rust]
       1) generate JobId (UUID)
       2) spawn Tokio task
       3) emit download-started
       4) launch yt-dlp subprocess
    -> useQueueEvents receives event
    -> queueStore.addJob()
    -> QueuePage updates
    -> [Rust] periodic download-progress events
    -> queueStore.updateJob()
    -> ActiveQueueList updates in real time
    -> [Rust] emit download-completed or download-failed
    -> queueStore.updateJob()
    -> item moves to completed/failed section
```

Queue state transitions:
1. Job inserted (`pending`)
2. Status `pending -> downloading -> completed/failed`
3. Progress updates from `0%` to terminal state

### 2. Job Control Workflow (Queue Domain)

#### Pause

```text
QueuePage -> pause action
    -> queueActions.pauseJob(jobId)
    -> desktopClient.pauseJob(jobId)
    -> Tauri invoke("pause_job")
    -> [Rust] signal subprocess pause + emit download-paused
    -> queueStore.updateJob(status="paused")
```

#### Resume

```text
QueuePage -> resume action
    -> queueActions.resumeJob(jobId)
    -> desktopClient.resumeJob(jobId)
    -> Tauri invoke("resume_job")
    -> [Rust] resume subprocess + emit download-resumed
    -> queueStore.updateJob(status="downloading")
```

#### Cancel

```text
QueuePage -> cancel action
    -> confirmation modal
    -> queueActions.cancelJob(jobId)
    -> desktopClient.cancelJob(jobId)
    -> Tauri invoke("cancel_job")
    -> [Rust] terminate subprocess + cleanup partial files + emit download-failed
    -> queueStore.updateJob(status="failed")
```

### 3. Settings Workflow (Settings Domain)

```text
SettingsPage
    -> user changes values
    -> settingsStore updates local state
```

Three main scenarios:

1. **Download directory selection**
- `desktopClient.pickDownloadDir()`
- backend opens native folder picker
- selected path returned and committed to store

2. **Defaults updates**
- User changes format/options in controls
- Store updates immediately for local UI consistency

3. **Persist settings**
- `desktopClient.setSettings(settings)`
- backend writes settings file
- success feedback shown in UI

## State Management Pattern

### Setup Store
- current URL input
- analysis result payload
- analysis loading flag
- selected format

### Queue Store
- active jobs and completed jobs
- per-job status/progress/speed/eta
- derived queue stats for summary cards

### Settings Store
- download directory
- default output format and options
- UI language and user preferences

## Command and Event Contracts

### Command Direction
- Frontend invokes typed command wrappers in `desktopClient`
- Backend validates input and executes side effects

### Event Direction
- Backend emits progress/status events
- `useQueueEvents` transforms events into store mutations
- UI reacts via standard React re-render cycle

## Error and Recovery Path

1. Command failures return typed errors
2. Store receives failure state and message
3. UI shows toasts and actionable retry options
4. Retry path reuses existing action handlers without duplicated logic

## Performance Considerations in Data Flow

- Event-driven updates avoid polling overhead
- Query caching minimizes duplicate backend calls
- Fine-grained Zustand selectors reduce unnecessary re-renders
- Background tasks in Rust keep UI thread responsive
