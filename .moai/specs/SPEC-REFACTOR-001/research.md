# Research: lib.rs Refactoring Analysis

## Executive Summary

`src-tauri/src/lib.rs` is a 2,491-line monolithic Tauri backend containing all business logic, state management, download worker threads, dependency management, and 18 API command handlers in a single file. Clear domain boundaries exist (metadata, download, queue, settings, dependencies, diagnostics) that can be extracted into logical modules. Critical issues include a 311-line worker function, 8x duplicated mutex recovery code, 20+ magic status string comparisons, and zero test coverage.

---

## File Statistics

| Metric | Count |
|--------|-------|
| Total lines | 2,491 |
| Struct definitions | 13 |
| Enum definitions | 2 |
| Type aliases | 1 |
| Tauri commands (#[tauri::command]) | 18 |
| Public helper functions | 2 |
| Private helper functions | ~50+ |
| impl blocks | 0 |
| Constants | 14 |

---

## Tauri Command Inventory

| Line | Command | Domain | Description |
|------|---------|--------|-------------|
| 1866 | `analyze_url` | Metadata | Extract video metadata and available formats |
| 2036 | `check_duplicate` | Queue | Check if URL+quality already in queue |
| 2056 | `enqueue_job` | Queue | Add download to queue |
| 2104 | `pause_job` | Download/Worker | Pause active download |
| 2130 | `resume_job` | Download/Worker | Resume paused download |
| 2154 | `cancel_job` | Download/Worker | Cancel download |
| 2180 | `clear_terminal_jobs` | Queue | Remove completed/failed/canceled items |
| 2192 | `get_queue_snapshot` | Queue | Get current queue state |
| 2198 | `get_settings` | Settings | Retrieve app settings |
| 2204 | `get_dependency_bootstrap_status` | Dependencies | Get dependency installation progress |
| 2215 | `pick_download_dir` | Settings | Open folder picker dialog |
| 2221 | `set_settings` | Settings | Update app settings |
| 2234 | `run_diagnostics` | Diagnostics | Check tool availability and paths |
| 2334 | `check_update` | System | Stub for update checking |
| 2343 | `get_storage_stats` | Diagnostics | Get disk usage stats |
| 2369 | `delete_file` | Queue | Remove file from queue and disk |
| 2381 | `open_folder` | System | Open folder in file explorer |
| 2442 | `open_external_url` | System | Open URL in browser |

---

## Module Boundary Analysis

### Domain 1: Metadata (`src-tauri/src/metadata.rs`)

**Purpose**: YouTube URL analysis and quality option extraction.

**Contents**:
- Structs: `QualityOption` (lines 64-70), `AnalysisResult` (lines 73-86)
- Enums: `DownloadMode` (lines 58-61)
- Functions: `analyze_url()` command (lines 1866-2033, ~165 lines), `normalize_youtube_video_url()` (lines 1265-1299)
- Helpers: `parse_progress_percent()`, `parse_speed()`, `parse_eta()` (lines 1364-1384)

**Issues**: `analyze_url()` is 165+ lines with deeply nested JSON format parsing; heavy `.unwrap()` usage.

---

### Domain 2: Download Worker (`src-tauri/src/download.rs`)

**Purpose**: Core download execution, retry logic, progress tracking, output parsing.

**Contents**:
- Structs/Enums: `RetryStrategy` (lines 1070-1075), `ActiveProcess` (lines 203-207), `SharedRuntime` (lines 220-221)
- Functions: `start_worker_if_needed()` (lines 1552-1863, ~311 lines), `classify_download_error()` (lines 1077-1109), `retry_delay_ms_for_strategy()` (lines 1111-1119), `handle_download_output_line()` (lines 1399-1463), `kill_active_child_unchecked()` (lines 1465-1483), `terminate_child_with_grace_period()` (lines 1509-1544)

**Issues**: `start_worker_if_needed()` is an extreme 311-line function containing: shutdown check, queue polling, dependency waiting, path building, yt-dlp command construction (35+ args), process spawning, stdout/stderr thread spawning, process wait loop, exit status handling, file movement logic, retry strategy classification, and retry delay calculation — ALL IN ONE LOOP.

---

### Domain 3: Queue Management (`src-tauri/src/queue.rs`)

**Purpose**: Download queue state, persistence, and queue operations.

**Contents**:
- Structs: `QueueItem` (lines 90-111, 20 fields), `AppState` (lines 193-197), `SharedState` (lines 200-201)
- Commands: `enqueue_job`, `check_duplicate`, `cancel_job`, `pause_job`, `resume_job`, `clear_terminal_jobs`, `delete_file`, `get_queue_snapshot`
- Functions: `persist_queue()` (lines 891-899, fan_in=8), `queue_snapshot()` (lines 244-247), `scan_incomplete_markers()` (lines 1227-1263), `build_unique_output_path()` (lines 1321-1349)

**Issues**: Status string comparisons ("queued", "downloading", "paused", etc.) used in 20+ places without type safety.

---

### Domain 4: Settings & Persistence (`src-tauri/src/settings.rs`)

**Purpose**: App configuration, user preferences, and state persistence with recovery.

**Contents**:
- Structs: `AppSettings` (lines 120-125), `PersistedSettings` (lines 184-190), `DependencyRuntimeState` (lines 226-230)
- Functions: `load_settings_with_recovery()` (lines 932-984, ~52 lines), `load_queue_with_recovery()` (lines 1000-1058, ~60 lines), `persist_settings()` (lines 903-912), `write_atomic()` (lines 878-886, fan_in=4), `normalize_download_dir()` (lines 857-873)
- Commands: `set_settings`, `get_settings`, `pick_download_dir`

**Issues**: Recovery logic is duplicated between `load_settings_with_recovery()` and `load_queue_with_recovery()` with the same three-level fallback pattern (primary → backup → defaults).

---

### Domain 5: Dependencies (`src-tauri/src/dependencies.rs`)

**Purpose**: Bootstrap yt-dlp and ffmpeg, version checking, platform-specific installation.

**Contents**:
- Functions: `bootstrap_dependencies()` (lines 791-801), `ensure_ytdlp()` (lines 617-682, ~65 lines), `ensure_ffmpeg_available()` (lines 684-711), `install_ffmpeg_windows()` (lines 714-789, ~75 lines), `latest_ytdlp_version()` (lines 564-584), `download_file()` (lines 586-615), `wait_for_dependencies()` (lines 833-855)

**Issues**: Platform-specific code heavily conditional with `#[cfg]` attributes; version checking mixes HTTP and process execution concerns.

---

### Domain 6: Diagnostics (`src-tauri/src/diagnostics.rs`)

**Purpose**: System health checks, storage stats, folder/URL opening.

**Contents**:
- Functions: `run_diagnostics()` (lines 2234-2293, ~59 lines), `get_storage_stats()` (lines 2343-2366), `open_folder()` (lines 2381-2439, ~58 lines), `open_external_url()` (lines 2442-2475), `can_write_to_dir()` (lines 2295-2307), `calculate_directory_size()` (lines 2313-2331)
- Commands: `run_diagnostics`, `get_storage_stats`, `open_folder`, `open_external_url`, `check_update`

**Issues**: `open_folder()` and `open_external_url()` have nearly identical platform dispatch code (macOS/Windows/Linux).

---

### Domain 7: File Operations (`src-tauri/src/file_ops.rs`)

**Purpose**: File handling, atomic operations, path resolution, binary search.

**Contents**:
- Functions: `move_file_atomic()` (lines 1130-1187, ~57 lines), `resolve_downloaded_file_path()` (lines 1189-1222), `resolve_executable()` (lines 356-383), `run_command_capture()` (lines 413-539, ~126 lines), path helpers: `app_data_dir()`, `temp_downloads_root_dir()`, `queue_file_path()`, `settings_file_path()`, etc.

**Issues**: `run_command_capture()` is 126 lines with watchdog thread and timeout handling; binary resolution is uncached; path helpers are trivial but numerous.

---

### Domain 8: Utilities (`src-tauri/src/utils.rs`)

**Purpose**: Pure parsing and formatting helpers.

**Contents**:
- Functions: `parse_progress_percent()`, `parse_speed()`, `parse_eta()`, `append_download_log()`, `select_format_expression()`, `expected_extension()`, `sanitize_file_name()`, `normalize_youtube_video_url()`

**Issues**: Progress parsing is fragile regex-based string parsing; format expressions assume yt-dlp command-line format string semantics.

---

## Proposed Module Structure

```
src-tauri/src/
├── lib.rs             (~80 lines: mod declarations + run() + command handler registrations)
├── state.rs           (~50 lines: SharedState, SharedRuntime, lock recovery helpers)
├── types.rs           (~80 lines: DownloadStatus enum, CommandResult alias, error types)
├── utils.rs           (~80 lines: pure parsing/formatting helpers)
├── file_ops.rs        (~150 lines: atomic moves, path resolution, binary search, command runner)
├── dependencies.rs    (~200 lines: bootstrap, version checking, ffmpeg install)
├── diagnostics.rs     (~150 lines: system checks, storage stats, open folder/URL)
├── settings.rs        (~100 lines: AppSettings, recovery, persistence)
├── metadata.rs        (~180 lines: analyze_url, quality option parsing, DownloadMode)
├── queue.rs           (~200 lines: QueueItem, commands, snapshot, persistence)
└── download.rs        (~350 lines: worker threads, retry logic, process management)
```

**lib.rs reduction**: 2,491 → ~80 lines (96.8% reduction)

---

## Critical Code Quality Issues

### Issue 1: `start_worker_if_needed()` — SEVERELY BLOATED (Severity: HIGH)
**Location**: `lib.rs:1552-1863` (~311 lines)
**Description**: Entire download worker lifecycle in a single closure containing: shutdown check, queue polling, dependency waiting, path building, process spawning with 35+ args, stdout/stderr readers, wait loop, exit handling, file movement, retry logic.
**Impact**: Untestable, cyclomatic complexity ~30+, mutex recovery code repeated 8 times.

### Issue 2: Duplicated Mutex Recovery Pattern (Severity: HIGH)
**Location**: 8+ places (lines 1554, 1576, 1694, 1726, 1762, 1796, 1857, etc.)
**Description**: Identical pattern repeated verbatim:
```rust
let mut state = shared.lock().unwrap_or_else(|e| {
    eprintln!("[STABILITY] Mutex poisoned, recovering: {:?}", e);
    e.into_inner()
});
```
**Impact**: 40+ lines of copy-pasted code; inconsistent error messages.
**Fix**: `fn lock_or_recover<T>(mutex: &Arc<Mutex<T>>, context: &str) -> MutexGuard<T>`

### Issue 3: Magic Status String Values (Severity: MEDIUM)
**Location**: 20+ comparisons throughout file
**Description**: Status as String type with 6 magic values: "queued", "downloading", "paused", "canceled", "completed", "failed"
**Impact**: No compile-time validation; typos undetectable; refactoring requires grep-replace.
**Fix**: `enum DownloadStatus { Queued, Downloading, Paused, Canceled, Completed, Failed }`

### Issue 4: Long Functions (Severity: HIGH)
| Function | Lines | Module |
|----------|-------|--------|
| `start_worker_if_needed()` | 311 | download.rs |
| `run_command_capture()` | 126 | file_ops.rs |
| `analyze_url()` | 165 | metadata.rs |
| `load_queue_with_recovery()` | 60 | settings.rs |
| `ensure_ytdlp()` | 65 | dependencies.rs |
| `install_ffmpeg_windows()` | 75 | dependencies.rs |
| `open_folder()` | 58 | diagnostics.rs |
| `run_diagnostics()` | 59 | diagnostics.rs |

### Issue 5: Inconsistent Error Handling (Severity: MEDIUM)
**Mixed patterns**: `.unwrap()` (15+ places), `.unwrap_or_else(|_| unreachable!())`, `.map_err(|e| e.to_string())`, `.ok()?` — all used inconsistently with no custom error types.
**Impact**: Potential panics in production; poor error context.

### Issue 6: Missing Documentation (Severity: MEDIUM)
**Count**: Only 3 doc comments found across 18 commands and all public structs.
**Impact**: Opaque public API for future maintainers.

---

## Dependency Graph Between Modules

```
lib.rs
  └── orchestrates all modules

types.rs, utils.rs, state.rs
  └── no internal dependencies (pure types/helpers)

file_ops.rs
  └── depends on: utils.rs, state.rs

dependencies.rs
  └── depends on: file_ops.rs, utils.rs, state.rs

diagnostics.rs
  └── depends on: file_ops.rs, dependencies.rs, state.rs

settings.rs
  └── depends on: file_ops.rs, state.rs, utils.rs

metadata.rs
  └── depends on: dependencies.rs

queue.rs
  └── depends on: state.rs, file_ops.rs, download.rs (worker trigger)

download.rs
  └── depends on: queue.rs, dependencies.rs, file_ops.rs, utils.rs, state.rs
```

No circular dependencies exist in the proposed design.

---

## Recommended Refactoring Sequence

**Phase 1: Foundation (zero-risk)**
1. Extract `state.rs` — shared types + `lock_or_recover()` helper
2. Extract `types.rs` — `DownloadStatus` enum, `CommandResult` alias
3. Extract `utils.rs` — pure parsing functions

**Phase 2: Utilities (low-risk)**
4. Extract `file_ops.rs` — file and process operations
5. Extract `dependencies.rs` — bootstrap and version checking
6. Extract `diagnostics.rs` — system checks

**Phase 3: Domain Logic (medium-risk)**
7. Extract `settings.rs` — persistence and recovery
8. Extract `metadata.rs` — URL analysis
9. Extract `queue.rs` — queue operations

**Phase 4: Core Worker (high-risk, last)**
10. Extract `download.rs` — refactor worker function into smaller pieces
11. Update `lib.rs` — only `run()` and handler registration

**Phase 5: Polish**
12. Add `///` doc comments to all public items
13. Run `cargo test`, `cargo clippy`, `cargo fmt`
14. Benchmark for performance regressions

---

## Risks and Considerations

- **Circular dependencies**: No cycles in proposed design. Queue ↔ Download coupling via direct function call (not trait).
- **Worker thread complexity**: `start_worker_if_needed()` must be extracted last, after all dependencies are stable.
- **Platform-specific code**: `#[cfg(target_os)]` blocks must be preserved exactly; test on all targets.
- **Mutex poisoning**: Centralizing `lock_or_recover()` must use same recovery behavior as current code to avoid regressions.
- **Atomic file writes**: `write_atomic()` pattern is correct but must be preserved in `file_ops.rs`.

---

Date: 2026-03-01
Analyzed by: MoAI Explore Agent
