# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Automated Release Command (SPEC-UPDATE-001)**: New `npm run release` command for automated versioning and publishing
  - `scripts/release.js` implements semantic versioning with `--patch`, `--minor`, `--major`, `--version X.Y.Z` flags
  - Validates clean git working tree before proceeding with release
  - Atomically updates `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml` versions
  - Creates git commit with `chore(release): v{version}` message, applies version tag, and pushes to remote
  - Registered as npm script: `"release": "node scripts/release.js"` in package.json

### Fixed

- **Auto-Update Verification (SPEC-UPDATE-001)**: `check_update()` now queries GitHub Releases API instead of returning stub value
  - Calls GitHub Releases API (`https://api.github.com/repos/SayBGM/TubeExtract/releases/latest`) to fetch latest version information
  - Returns accurate `hasUpdate` boolean, `latestVersion` string, and download URL
  - Graceful error handling: returns `hasUpdate: false` on network failures, timeouts, or JSON parsing errors
  - No app crashes on API failures; errors logged and safely handled
  - Maintains browser-based download flow: users open GitHub release page directly instead of in-app update
- **CI/CD Release Pipeline**: Updated `.github/workflows/release.yml` to synchronize `src-tauri/Cargo.toml` version
  - Previously only `package.json` and `src-tauri/tauri.conf.json` were updated during CI release
  - Now all three files are kept in sync with the git version tag

### Refactored

- **lib.rs Modularization (SPEC-REFACTOR-001)**: Split 2,491-line monolithic lib.rs into 10 focused modules
  - `state.rs` - Mutex recovery helper (`lock_or_recover`) and shared state types
  - `types.rs` - `CommandResult<T>` type alias for unified command return types
  - `utils.rs` - Pure parsing helpers (progress percentage, speed, ETA calculation, file name sanitization)
  - `file_ops.rs` - Atomic file I/O operations, process execution, and executable resolution
  - `dependencies.rs` - yt-dlp and ffmpeg bootstrap with platform-specific installation and version checking
  - `diagnostics.rs` - System diagnostics, update checks, and storage statistics (5 Tauri commands)
  - `settings.rs` - Application settings persistence with automatic backup recovery and validation
  - `metadata.rs` - URL analysis and quality option discovery for video/audio formats
  - `queue.rs` - Download queue management with state persistence and event emission (8 Tauri commands)
  - `download.rs` - Core download worker with intelligent retry strategy and concurrent download support
  - All 95 tests passing: 21 (phase 1) + 17 (phase 2) + 19 (phase 3) + 38 (stability)
  - Public doc comments added to all exported functions, types, and modules
  - `lib.rs` reduced to 130 lines (from 2,491) with improved readability and maintainability

### Added

- **Data Integrity & Corruption Recovery (SPEC-STABILITY-002)**
  - Atomic file write operations to prevent data corruption during application crashes
  - Queue file corruption recovery: automatically restores from backup with user notification via `queue-corruption-recovered` event
  - Settings file corruption recovery: automatically restores from backup with user notification via `settings-corruption-recovered` event
  - Cross-device atomic download completion: `.incomplete` marker pattern prevents corrupt downloads on different volumes/partitions
  - Startup scan for incomplete file transfers: detects orphaned `.incomplete` markers and marks affected queue items as failed for retry

### Fixed

- **Rust Backend Stability Improvements (SPEC-STABILITY-001)**
  - Removed Mutex poisoning panics (9 locations) - now recovers with `into_inner()` and continues operation
  - Implemented graceful shutdown for worker threads via mpsc channel with JoinHandle tracking
  - Added watchdog timeout enforcement for `run_command_capture()` function with deadline-based process killing
  - Replaced hard `.expect()` panic in Tauri startup with error logging and `std::process::exit(1)`
  - Implemented process termination verification with 500ms grace period + Windows `taskkill /F /T` for child process cleanup
  - Enhanced React Query resilience settings: `retry: 3`, `staleTime: 30000ms`, exponential backoff strategy

### Changed

- React Query now uses exponential backoff (1s, 2s, 4s, max 30s) for automatic retries on network failures
- Query cache staleness increased from unspecified to 30 seconds to reduce unnecessary background refetches
- All Mutex lock failures in worker threads now log recovery events with `[STABILITY]` prefix for diagnostics

### Added

- Characterization tests for stability fixes in `src-tauri/tests/stability_tests.rs`
  - Tests verify Mutex poison recovery behavior
  - Tests confirm grace period timeout duration (500ms)
  - Tests validate QueryClient configuration after updates

### Performance

- Reduced unnecessary React Query background refetches by implementing 30s staleTime
- Improved process termination handling with platform-specific Windows child process cleanup
- Eliminated blocking panic scenarios that previously crashed the entire application

---

## Version Numbering

### Major.Minor.Patch Format

- **Major**: Breaking changes or significant feature releases
- **Minor**: New features, improvements, stability enhancements
- **Patch**: Bug fixes, documentation updates
