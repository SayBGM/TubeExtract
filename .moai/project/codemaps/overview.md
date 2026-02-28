# TubeExtract Architecture Overview

## Project Introduction

**TubeExtract** (yt-downloder) is a desktop application for downloading YouTube content locally. It combines a modern web UI with a Rust backend for performance, reliability, and a user-friendly experience.

## Core Architecture Characteristics

### Multi-Layer Architecture

TubeExtract is organized into five clearly separated layers:

1. **React UI Layer**
- User interface and interaction handling
- Three primary domains: Setup, Queue, Settings
- Route management via React Router

2. **State Management Layer**
- React Query for async data fetching and caching
- Zustand for client state (`setupStore`, `queueStore`, `settingsStore`)
- Domain hooks for orchestration (`useQueueEvents`, `useDependencyBootstrap`, etc.)

3. **Desktop Bridge Layer**
- `desktopClient.ts` manages Tauri IPC communication
- Typed command invocation surface for 18 Tauri commands

4. **Tauri Runtime Layer (Rust Backend)**
- Core business logic in `src-tauri/src/lib.rs`
- Command handling, process management, filesystem operations

5. **External Services Layer**
- `yt-dlp` for metadata extraction and downloading
- `FFmpeg` for media processing
- GitHub API for update checks

## Domain-Driven Design

### Setup Domain (URL Analysis and Format Selection)

**Responsibilities**
- Validate YouTube URLs
- Extract metadata (title, description, duration, formats)
- Present selectable formats
- Detect duplicate downloads

**Main Components**
- `SetupPage.tsx`
- `SetupUrlForm`
- `SetupAnalysisResult`
- `SetupAnalyzingState`
- `SetupHeader`

### Queue Domain (Download Job Management)

**Responsibilities**
- Manage download queue lifecycle
- Track active/completed/failed states
- Pause, resume, and cancel jobs
- Show real-time progress updates

**Main Components**
- `QueuePage.tsx`
- `queueActions.ts`
- `ActiveQueueList`
- `CompletedQueueList`
- `QueueSummaryCards`

### Settings Domain (Application Configuration)

**Responsibilities**
- Manage download directory and defaults
- Run dependency diagnostics
- Check for updates

**Main Components**
- `SettingsPage.tsx`
- `SettingsDiagnosticsSection`
- `SettingsUpdateSection`
- `SettingsDefaultsSection`

## Key Design Decisions

1. **Tauri over Electron**
- Lower memory footprint and smaller bundles
- Rust safety and performance benefits

2. **DDD-Oriented Feature Separation**
- Clear boundaries between Setup, Queue, and Settings
- Better maintainability and team scalability

3. **React Query + Zustand**
- React Query handles async/server state
- Zustand keeps client-side state simple and fast

4. **Event-Driven Real-Time Updates**
- Backend progress is pushed via Tauri events instead of polling
- Better UX and lower resource overhead

## System Boundaries

### Frontend Boundary
- Node.js/npm ecosystem
- React + TypeScript application

### Backend Boundary
- Rust/Tauri runtime
- External process management (`yt-dlp`, `FFmpeg`)
- Filesystem operations

### Communication Boundary
- Tauri IPC for command requests (frontend -> backend)
- Tauri events for live updates (backend -> frontend)

## Technology Summary

### Frontend
- React 19.2.0, TypeScript 5.9.3
- Vite 7.3.1, Tailwind CSS 4.2.0
- React Query 5.90.21, Zustand 5.0.11
- React Router DOM 7.13.0

### Backend
- Tauri 2.0 (Rust)
- Tokio (async runtime)
- Reqwest 0.12 (HTTP)
- UUID, Regex, Zip utility libraries

### External Dependencies
- `yt-dlp`: metadata extraction and downloading
- `FFmpeg`: media processing/conversion
- GitHub API: update information

## Performance Characteristics

### Optimization Strategy
1. Bundle optimization with Vite code splitting
2. React Query caching and background refresh
3. Normalized state updates via Zustand
4. Event-driven updates to avoid polling costs

### Scalability
- Domain-based parallel development
- Modular architecture for feature expansion
- Command-oriented backend that is easy to extend
