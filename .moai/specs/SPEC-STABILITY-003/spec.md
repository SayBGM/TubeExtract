---
id: SPEC-STABILITY-003
version: 1.0.0
status: completed
created: 2026-03-01
updated: 2026-03-01
author: backgwangmin
priority: high
domain: stability
tags: [yt-dlp, download-stability, network-resilience, format-selection]
---

# SPEC-STABILITY-003: Download Resilience & Audio Fix

## Environment

- **Platform**: Tauri 2.0 desktop application (macOS, Windows, Linux)
- **Language**: Rust (src-tauri/src/lib.rs)
- **Download Engine**: yt-dlp subprocess
- **Development Mode**: DDD (ANALYZE-PRESERVE-IMPROVE)

## Assumptions

| # | Assumption | Confidence | Risk if Wrong |
|---|-----------|------------|---------------|
| A1 | yt-dlp supports all proposed flags in current bundled version | High | Flags silently ignored or error |
| A2 | `--concurrent-fragments 4` is safe for typical connections | High | Bandwidth saturation on slow connections |
| A3 | `--throttled-rate 100K` threshold is appropriate for YouTube | Medium | False positives on slow connections |
| A4 | `best[acodec!=none]` fallback reliably selects audio-bearing formats | High | Edge cases with unusual format availability |

## Problem Analysis

### Issue #1: Audio Missing on Fallback Format (FIXED)

**Location**: `src-tauri/src/lib.rs`, `select_format_expression` (line 1286-1297)

**Root Cause**: Format expression `{quality_id}+bestaudio/best` falls back to `best` which
on YouTube can return video-only DASH streams without audio when FFmpeg merge fails.

**Fix Applied**: Changed to `{quality_id}+bestaudio/best[acodec!=none]/best`

---

### Issue #2: No Socket Timeout

**Location**: `src-tauri/src/lib.rs`, download args (line 1585-1608)

**Current Behavior**: No `--socket-timeout` passed to yt-dlp. On slow or unresponsive
connections, yt-dlp hangs indefinitely. The outer Rust retry loop never triggers because
the subprocess never exits.

**Impact**: Application appears frozen. User must manually cancel and retry.

---

### Issue #3: No Fragment-Level Retry

**Location**: `src-tauri/src/lib.rs`, download args

**Current Behavior**: No `--fragment-retries` passed to yt-dlp. When downloading
HLS/DASH streams (YouTube's primary format), individual fragment failures cause the
entire download to fail. The outer Rust retry loop then restarts from 0%.

**Impact**: Any transient network issue during a long download causes full restart.

---

### Issue #4: No Throttling Detection

**Location**: `src-tauri/src/lib.rs`, download args

**Current Behavior**: No `--throttled-rate` passed to yt-dlp. YouTube sometimes
throttles specific format downloads to very low speeds (e.g., 10-50KB/s). Without
detection, the download completes eventually but at unacceptable speed.

**Impact**: Downloads stall at extremely low speeds without any indication or recovery.

---

### Issue #5: No Extractor-Level Retry

**Location**: `src-tauri/src/lib.rs`, download args

**Current Behavior**: No `--extractor-retries` passed to yt-dlp. YouTube API rate
limiting or transient extraction failures cause immediate download failure rather than
a retry at the extractor level.

**Impact**: Temporary YouTube API issues result in failed downloads that require
manual retry.

---

### Issue #6: No Concurrent Fragment Download

**Location**: `src-tauri/src/lib.rs`, download args

**Current Behavior**: yt-dlp downloads DASH fragments sequentially by default.
Concurrent fragment downloading would improve both speed and resilience (failed
fragments can be retried independently).

**Impact**: Slower download speeds and higher sensitivity to individual fragment failures.

---

## Requirements

### REQ-001: Audio Format Fallback Fix (COMPLETED)

**Priority**: CRITICAL | **Status**: ✅ Already Fixed

**Event-Driven Requirement**:
- **When** `select_format_expression` is called with a video quality_id not containing `+`,
  **then** the format expression SHALL be `{quality_id}+bestaudio/best[acodec!=none]/best`
  ensuring audio is present in all fallback scenarios.

---

### REQ-002: Socket Timeout

**Priority**: HIGH

**Event-Driven Requirement**:
- **When** a download job is started, **then** yt-dlp SHALL be invoked with
  `--socket-timeout 30` to prevent indefinite hangs on unresponsive connections.

**Unwanted Behavior**:
The system SHALL NOT allow yt-dlp to wait indefinitely for a server response.

---

### REQ-003: Fragment-Level Retry

**Priority**: HIGH

**Event-Driven Requirement**:
- **When** a download job is started, **then** yt-dlp SHALL be invoked with
  `--fragment-retries 10` to retry individual HLS/DASH fragment downloads up to 10 times
  before considering the fragment failed.

**Unwanted Behavior**:
A single fragment failure SHALL NOT cause the entire download to fail and restart from 0%.

---

### REQ-004: Throttling Detection

**Priority**: MEDIUM

**Event-Driven Requirement**:
- **When** a download job is started, **then** yt-dlp SHALL be invoked with
  `--throttled-rate 100K` so that yt-dlp detects YouTube speed throttling (below 100KB/s)
  and automatically switches to an alternative format or retries.

---

### REQ-005: Extractor Retry

**Priority**: MEDIUM

**Event-Driven Requirement**:
- **When** a download job is started, **then** yt-dlp SHALL be invoked with
  `--extractor-retries 5` to retry extractor-level failures (YouTube API errors) up to
  5 times before failing.

---

### REQ-006: Concurrent Fragment Download

**Priority**: MEDIUM

**Event-Driven Requirement**:
- **When** a download job is started, **then** yt-dlp SHALL be invoked with
  `--concurrent-fragments 4` to download up to 4 DASH fragments simultaneously,
  improving throughput and resilience.

---

## Implementation Scope

### Files to Modify

| File | Change |
|------|--------|
| `src-tauri/src/lib.rs` | Add yt-dlp args to download job startup |
| `src-tauri/tests/stability_tests.rs` | Add characterization tests for new args |

### Args to Add

```rust
// Socket timeout: prevent indefinite hang
"--socket-timeout", "30",

// Fragment retry: HLS/DASH resilience
"--fragment-retries", "10",

// Throttling detection
"--throttled-rate", "100K",

// Extractor retry: YouTube API errors
"--extractor-retries", "5",

// Concurrent fragments: speed + resilience
"--concurrent-fragments", "4",
```

### Args NOT to Add (rationale)

| Arg | Reason |
|-----|--------|
| `--retries N` | Outer Rust loop already handles job-level retries |
| `--sleep-interval` | Not needed; YouTube rarely rate-limits yt-dlp in standard use |
| `--cookies-from-browser` | Requires user permission; out of scope |

## Constraints

- **C1**: No new Rust crate dependencies
- **C2**: Args must be compatible with the yt-dlp version bundled by the app
- **C3**: Args must be prepended before the URL in the args vector
- **C4**: Characterization tests must verify args are present in the built command

## Traceability

| Requirement | Issue | Risk Level | Test Scenario |
|-------------|-------|------------|---------------|
| REQ-001 | Audio missing on fallback | CRITICAL | select_format_expression returns correct expr |
| REQ-002 | No socket timeout | HIGH | args contains --socket-timeout 30 |
| REQ-003 | No fragment retry | HIGH | args contains --fragment-retries 10 |
| REQ-004 | No throttle detection | MEDIUM | args contains --throttled-rate 100K |
| REQ-005 | No extractor retry | MEDIUM | args contains --extractor-retries 5 |
| REQ-006 | No concurrent fragments | MEDIUM | args contains --concurrent-fragments 4 |

## Implementation Notes

### Completion Status: ✅ COMPLETED

All requirements have been successfully implemented.

### Requirement Fulfillment

**REQ-001: Audio Format Fallback Fix**
- ✅ `select_format_expression()` now returns `{quality_id}+bestaudio/best[acodec!=none]/best`
- Ensures audio-bearing formats in all fallback scenarios

**REQ-002: Socket Timeout**
- ✅ `--socket-timeout 30` added to yt-dlp download args

**REQ-003: Fragment-Level Retry**
- ✅ `--fragment-retries 10` added to yt-dlp download args

**REQ-004: Throttling Detection**
- ✅ `--throttled-rate 100K` added to yt-dlp download args

**REQ-005: Extractor Retry**
- ✅ `--extractor-retries 5` added to yt-dlp download args

**REQ-006: Concurrent Fragment Download**
- ✅ `--concurrent-fragments 4` added to yt-dlp download args

### Test Coverage

- 28/28 tests passing (+4 new stability tests)
- Code quality: ✅ clippy clean

### Files Modified

- `src-tauri/src/lib.rs` (select_format_expression fix + 5 new yt-dlp args)
- `src-tauri/tests/stability_tests.rs` (+4 new characterization tests)
