# Stability Research Report: TubeExtract Codebase

**Date**: 2026-03-01
**Analyzed By**: MoAI Explore Agent
**Project**: TubeExtract (yt-downloder) v0.0.0
**Tech Stack**: Tauri 2.0 + React 19 + TypeScript + Rust

---

## Executive Summary

TubeExtract is a Tauri 2.0 desktop application with a React/TypeScript frontend and Rust backend. The codebase demonstrates solid architectural patterns but contains **critical stability risks** primarily in error handling and resource management. Most issues are concentrated in the Rust backend's process lifecycle management and state synchronization.

---

## CRITICAL ISSUES (P0/P1)

### 1. Mutex Poisoning Panics in Production Code
- **File**: `src-tauri/src/lib.rs`
- **Lines**: 1034, 1155, 1205, 1248, 1310, 1340, 1372, 1877
- **Issue**: Frequent `panic!("state lock poisoned")` on `Mutex::lock()` failures
- **Code Pattern**:
```rust
let mut state = shared.lock().unwrap_or_else(|_| panic!("state lock poisoned"));
```
- **Impact**: App crashes immediately if a mutex becomes poisoned. This can occur if a thread panics while holding a lock. In production, this is a cascading failure scenario.
- **Risk Level**: CRITICAL

### 2. Thread Spawning Without Completion Guarantees
- **File**: `src-tauri/src/lib.rs`
- **Lines**: 1170-1388 (worker loop), 1287-1304 (stdout/stderr threads), 1647, 1697
- **Issue**: Multiple background threads spawned with:
  - No tracking of thread handles
  - No join timeout
  - Worker loop never terminates cleanly (infinite loop with only queue-based exit)
  - Stdout/stderr reader threads could accumulate if process spawning loops
- **Code Pattern**:
```rust
std::thread::spawn(move || loop { ... });  // No way to stop this
let _ = handle.join();  // Ignores thread panic
```
- **Impact**: Resource exhaustion if many jobs are queued quickly.
- **Risk Level**: CRITICAL

### 3. Unimplemented Timeout Parameter in Command Execution
- **File**: `src-tauri/src/lib.rs`
- **Lines**: 402-436 (`run_command_capture` function)
- **Issue**: `_timeout_ms` parameter is declared but never used. All command executions block indefinitely.
- **Current Code**:
```rust
fn run_command_capture(
    app: &AppHandle,
    command: &str,
    args: &[&str],
    _timeout_ms: u64,  // <-- UNUSED - timeout never actually applied
) -> CommandCaptureResult {
    // ... no timeout implementation
}
```
- **Impact**: If yt-dlp or ffmpeg hang, the entire worker thread becomes unresponsive. Frontend appears frozen. 15s analyze timeout and 10s diagnostics timeout are NOT enforced.
- **Risk Level**: CRITICAL

### 4. Startup Failure Hard Panic
- **File**: `src-tauri/src/lib.rs`
- **Line**: 2061
- **Issue**: `expect()` on main Tauri run loop
```rust
.run(tauri::generate_context!())
.expect("error while running tauri application");  // Hard crash on failure
```
- **Impact**: Any Tauri initialization failure causes hard crash with minimal logging
- **Risk Level**: CRITICAL

---

## HIGH PRIORITY ISSUES (P2)

### 5. Queue File Corruption Not Handled
- **File**: `src-tauri/src/lib.rs`
- **Lines**: 826-835 (load_queue function)
- **Issue**: If `queue_state.json` is malformed, all data is silently discarded
- **Impact**: Data loss without notification to user
- **Risk Level**: HIGH

### 6. Process Termination Without Verification
- **File**: `src-tauri/src/lib.rs`
- **Lines**: 1124-1138 (terminate_child_with_grace_period)
- **Issue**: 500ms grace period, then force kill. On Windows, may orphan child processes.
- **Impact**: Background yt-dlp processes continue running after "cancel"
- **Risk Level**: HIGH

### 7. Dependency Bootstrap Timeout is Blocking
- **File**: `src-tauri/src/lib.rs`
- **Lines**: 730-751 (wait_for_dependencies)
- **Issue**: 600-iteration busy-wait polling (60 seconds) with no user feedback
- **Impact**: App appears frozen during first cold start
- **Risk Level**: HIGH

### 8. No Atomic Operations for Download Completion
- **File**: `src-tauri/src/lib.rs`
- **Lines**: 1346-1359
- **Issue**: File move operation is not atomic - power failure mid-move corrupts download
- **Impact**: Corrupted downloaded files with no recovery
- **Risk Level**: HIGH

### 9. No Binary Checksum Verification
- **File**: `src-tauri/src/lib.rs`
- **Lines**: 629-639 (download_file callback)
- **Issue**: Downloaded yt-dlp/ffmpeg binaries not checksummed before execution
- **Impact**: Security risk + stability risk from corrupted binaries
- **Risk Level**: HIGH

### 10. React Query Overly Aggressive Defaults
- **File**: `src/renderer/lib/queryClient.ts`
- **Lines**: 3-10
- **Issue**: Only 1 retry, no refetchOnWindowFocus for critical queries
- **Impact**: Stale data, poor network resilience
- **Risk Level**: HIGH

### 11. Download Progress Parsing Fragility
- **File**: `src-tauri/src/lib.rs`
- **Lines**: 995-1008
- **Issue**: Progress parsing assumes fixed yt-dlp output format with no versioning
- **Impact**: yt-dlp version update silently breaks progress tracking
- **Risk Level**: HIGH

---

## MEDIUM PRIORITY ISSUES (P3)

### 12. Error Messages Mixed Language (Korean hardcoded in Rust)
- **Files**: Multiple in `src-tauri/src/lib.rs`
- **Lines**: 1361, 1688, 1599, 606
- **Issue**: Frontend should control all user-facing messages; Rust strings are not i18n-able
- **Risk Level**: MEDIUM

### 13. Download Log Truncation Without Warning
- **File**: `src-tauri/src/lib.rs`
- **Lines**: 1010-1021
- **Issue**: Silent truncation of log lines (keeps only last 120 lines)
- **Risk Level**: MEDIUM

### 14. No Recovery for Corrupted Settings
- **File**: `src-tauri/src/lib.rs`
- **Lines**: 786-800
- **Issue**: Malformed settings.json silently falls back to defaults; user loses preferences
- **Risk Level**: MEDIUM

### 15. URL Validation Inconsistency
- **File**: `src/renderer/domains/setup/useSetupActions.ts`
- **Issue**: Client-side and backend validation rules diverge
- **Risk Level**: MEDIUM

### 16. No Retry Logic for Dependency Downloads
- **File**: `src-tauri/src/lib.rs`
- **Lines**: 561-571
- **Issue**: `download_file` has no retry logic; network hiccup fails entire bootstrap
- **Risk Level**: MEDIUM

### 17. Minimal Test Coverage
- **Test Files Found**: Only 3 basic test files
  - `src/renderer/domains/queue/queueActions.test.ts`
  - `src/renderer/domains/setup/useSetupActions.test.tsx`
  - `src/renderer/domains/settings/SettingsPage.test.tsx`
- **CI/CD**: test.yml only runs `npm run test` (Rust tests not included)
- **Risk Level**: MEDIUM

### 18. AppErrorBoundary Shows Generic Message Only
- **File**: `src/renderer/components/AppErrorBoundary.tsx`
- **Lines**: 17-36
- **Issue**: No error details logged, hardcoded Korean message, no crash reporting
- **Risk Level**: MEDIUM

### 19. No Polling Timeout for Queue Events (Web Mode)
- **File**: `src/renderer/hooks/useQueueEvents.ts`
- **Lines**: 27-35
- **Issue**: Web mode polling runs indefinitely at 300ms intervals
- **Risk Level**: MEDIUM

### 20. Missing Schema Validation on Desktop Client Responses
- **File**: `src/renderer/lib/desktopClient.ts`
- **Lines**: 219-223
- **Issue**: Type assertions without runtime schema validation
- **Risk Level**: MEDIUM

---

## Architecture Observations

### Positive Patterns
1. Grace period termination design (line 1124) is thoughtful
2. Event-driven UI updates via Tauri events (not polling)
3. Clean domain-separated React structure (setup/queue/settings)
4. Persistent queue across sessions
5. Async dependency bootstrap (non-blocking startup)

### Areas of Concern
1. Single worker thread limits throughput (intentional but fragile)
2. File-based JSON persistence vulnerable to corruption
3. No circuit breaker for backend failures
4. No exponential backoff for retries
5. Global mutex serializes all operations under load

---

## SPEC Recommendations

| Priority | SPEC ID | Title | Issues Covered |
|----------|---------|-------|----------------|
| 1 (Critical) | SPEC-STABILITY-001 | Rust Backend Error Handling | P0: #1, #2, #3, #4 |
| 2 (High) | SPEC-STABILITY-002 | Data Integrity & Persistence | P1: #5, #8, #14 |
| 3 (High) | SPEC-STABILITY-003 | Dependency Management Hardening | P1: #7, #9, #16 |
| 4 (Medium) | SPEC-STABILITY-004 | Test Coverage & CI/CD | P2: #17 |
| 5 (Medium) | SPEC-STABILITY-005 | Error UX & Observability | P2: #12, #15, #18 |

---

## Statistics

- **Total Rust LOC**: 2,062
- **Total TypeScript LOC**: ~3,000+
- **Test Files**: 3 (minimal coverage)
- **Critical Issues**: 4
- **High Priority Issues**: 7
- **Medium Priority Issues**: 9
