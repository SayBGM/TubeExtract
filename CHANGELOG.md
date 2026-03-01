# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
