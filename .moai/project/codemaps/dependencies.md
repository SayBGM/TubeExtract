# TubeExtract Dependency Graph

## Dependency Overview

TubeExtract is split into a frontend (TypeScript/React) and a backend (Rust/Tauri), with clear boundaries and explicit integration points.

## Frontend Dependencies (npm)

### Core
- `react@19.2.0`: UI rendering
- `react-dom@19.2.0`: DOM integration
- `typescript@5.9.3`: static typing
- `vite@7.3.1`: dev/build tooling
- `react-router-dom@7.13.0`: client-side routing

### State Management
- `@tanstack/react-query@5.90.21`: async/server state and caching
- `zustand@5.0.11`: client state stores (`setup`, `queue`, `settings`)

### Forms and Input
- `react-hook-form@7.71.2`: form state + validation flow

### UI and Styling
- `tailwindcss@4.2.0`: utility CSS system
- `@radix-ui/react-select@2.2.6`: accessible select primitives
- `motion@12.34.3`: animation behavior
- `sonner@2.0.7`: toast notifications

### i18n and Desktop Integration
- `i18next@25.8.13`: localization framework
- `@tauri-apps/api@2.10.1`: Tauri runtime bridge

## Backend Dependencies (Cargo/Rust)

### Core Runtime
- `tauri@2.0`: desktop runtime and command/event system
- `tokio@1.x`: async runtime for subprocess and task management

### Data and Networking
- `serde@1.x`: serialization/deserialization
- `reqwest@0.12`: HTTP client for remote checks

### Utility Libraries
- `chrono@0.4`: date/time handling
- `uuid@1.x`: unique job IDs
- `regex@1.x`: parsing and validation
- `zip@2.x`: archive support
- `dirs@6.0`: standard directory path resolution
- `rfd@0.15`: native file dialog integration

## External Process Dependencies

### `yt-dlp` (Required)
- Metadata extraction (`--dump-json`)
- Download execution by format and output path
- Critical dependency for core product behavior

### `FFmpeg` (Optional but important)
- Media conversion/post-processing
- Validation and transformation workflows

### GitHub API (Optional)
- Release/version metadata for update checks

## Dependency Flow

### Frontend

```text
App.tsx
├── React + TypeScript
├── React Router -> Setup/Queue/Settings pages
├── State layer
│   ├── Zustand stores
│   └── React Query cache
├── UI layer
│   ├── Tailwind CSS
│   ├── Radix UI
│   └── motion
├── desktopClient -> @tauri-apps/api
├── i18next
└── sonner notifications
```

### Backend

```text
main.rs
└── lib.rs
    ├── Tauri runtime
    ├── Tokio async execution
    ├── Command handlers
    ├── Event emitters
    ├── serde for JSON
    ├── reqwest for HTTP
    ├── uuid/regex/chrono/dirs/zip
    └── External subprocesses
        ├── yt-dlp (required)
        └── FFmpeg (optional)
```

### Domain-Level Relationships

**Setup Domain**
```text
SetupPage -> setupStore <- setupQueries
         -> useSetupActions -> desktopClient -> analyze_url/check_duplicate
```

**Queue Domain**
```text
QueuePage -> queueStore <- queueActions -> desktopClient -> enqueue/pause/resume/cancel
         -> useQueueEvents <- Tauri events from backend
```

**Settings Domain**
```text
SettingsPage -> settingsStore <- settingsQueries
             -> desktopClient -> get_settings/set_settings/run_diagnostics
```

## Versioning Strategy

### Major Version Stability
- Track major lines: React 19.x, TypeScript 5.x, Tauri 2.x, Tokio 1.x, Serde 1.x

### Minor/Patch Strategy
- npm: controlled semver updates with CI validation
- Cargo: explicit version management

### Maintenance Plan
- Monthly security review
- Quarterly update review
- CI-based regression checks on dependency updates
