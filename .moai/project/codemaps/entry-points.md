# TubeExtract Entry Points and Commands

## Application Entry Points

### Frontend Entry Points

#### `index.html`
- **Location**: Project root
- **Role**: HTML bootstrap file

#### `src/main.tsx`
- **Role**: React app initialization
- **Responsibilities**:
  1. Mount root React tree
  2. Initialize `App.tsx`
  3. Configure React Query client
  4. Initialize Zustand stores
  5. Initialize i18n resources

#### `src/App.tsx`
- **Role**: Root composition and route wiring
- **Core layout**:

```tsx
<BrowserRouter>
  <AppErrorBoundary>
    <DependencyBootstrapOverlay>
      <div className="app-layout">
        <Sidebar />
        <Routes>
          <Route path="/setup" element={<SetupPage />} />
          <Route path="/queue" element={<QueuePage />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Routes>
      </div>
    </DependencyBootstrapOverlay>
  </AppErrorBoundary>
</BrowserRouter>
```

### Backend Entry Points

#### `src-tauri/src/main.rs`
- **Role**: Main Tauri startup entry
- **Responsibilities**:
  1. Initialize Tauri builder
  2. Create app window(s)
  3. Configure menus
  4. Start runtime and command handlers

#### `src-tauri/src/lib.rs`
- **Role**: Command and event implementation
- **Includes**:
  - 18 command handlers
  - event emission for progress/status
  - business logic and process control

## Routing Structure

### `/setup` (default)
- **Component**: `SetupPage.tsx`
- **Purpose**: URL input and format selection
- **State owner**: `setupStore`

### `/queue`
- **Component**: `QueuePage.tsx`
- **Purpose**: Manage active and completed jobs
- **State owner**: `queueStore`
- **Realtime updates**: `useQueueEvents`

### `/settings`
- **Component**: `SettingsPage.tsx`
- **Purpose**: Configure app defaults and diagnostics
- **State owner**: `settingsStore`

## Tauri Commands (18)

### URL Analysis and Validation

1. `analyze_url`
- Parse and validate URL
- Execute `yt-dlp --dump-json`
- Return structured metadata

2. `check_duplicate`
- Search existing download assets
- Detect already-downloaded items

### Queue Management

3. `enqueue_job`
4. `pause_job`
5. `resume_job`
6. `cancel_job`
7. `clear_terminal_jobs`

### Filesystem and Navigation

8. `delete_file`
9. `open_folder`
10. `open_external_url`

### State and Settings Queries

11. `get_queue_snapshot`
12. `get_settings`
13. `get_storage_stats`

### Settings and Diagnostics Mutations

14. `pick_download_dir`
15. `set_settings`
16. `run_diagnostics`
17. `check_update`
18. `get_dependency_bootstrap_status`

## Request/Response Pattern

Frontend uses `desktopClient` wrappers to invoke commands via Tauri IPC. Backend emits lifecycle events to keep UI stores synchronized without polling.

### Typical Download Lifecycle

1. Setup domain calls `enqueue_job`
2. Backend emits `download-started`
3. Backend emits periodic `download-progress`
4. Backend emits either `download-completed` or `download-failed`
5. Queue store updates and UI re-renders
