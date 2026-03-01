---
id: SPEC-STABILITY-005
version: 1.0.0
status: completed
created: 2026-03-01
updated: 2026-03-01
author: backgwangmin
priority: high
domain: stability
tags: [retry-strategy, error-classification, network-resilience]
---

# SPEC-STABILITY-005: Smart Retry Strategy

## Environment

- **Platform**: Tauri 2.0 desktop application (macOS, Windows, Linux)
- **Language**: Rust (src-tauri/src/lib.rs)
- **Development Mode**: DDD (ANALYZE-PRESERVE-IMPROVE)

## Assumptions

| # | Assumption | Confidence | Risk if Wrong |
|---|-----------|------------|---------------|
| A1 | yt-dlp stderr output contains consistent error patterns | High | Classification may miss edge cases |
| A2 | HTTP 429 delay of 30-60s is sufficient before retry | Medium | May need exponential backoff |
| A3 | "Video unavailable" errors are permanent and should not retry | High | Rare cases where availability is transient |
| A4 | Network errors are transient and benefit from quick retry | High | Persistent outages will still exhaust retries |

## Problem Analysis

### Issue: All Errors Use Identical Retry Strategy

**Location**: `src-tauri/src/lib.rs`, `retry_delay_ms` (line 1058) and retry decision block (line 1763)

**Current Behavior**:
```rust
const RETRY_DELAY_TABLE_MS: [u64; 4] = [2000, 5000, 10000, 15000];

fn retry_delay_ms(attempt: usize) -> u64 {
    let idx = attempt.min(RETRY_DELAY_TABLE_MS.len().saturating_sub(1));
    RETRY_DELAY_TABLE_MS[idx]
}

// Retry decision (line 1763):
if attempt < max_retries {
    should_retry = true;
    // ... same delay regardless of error type
}
```

**Problems**:
1. **Permanent errors retry unnecessarily**: "Video unavailable", "Private video", HTTP 404 will never succeed but retry up to `max_retries` times, wasting time and bandwidth.
2. **Rate limit errors retry too quickly**: HTTP 429 (Too Many Requests) with a 2-5 second retry will immediately trigger another 429, compounding the problem.
3. **Transient network errors could retry faster**: Socket timeout / connection refused benefit from immediate retry (< 2s), not waiting 2-15 seconds.

**Impact**:
- Failed downloads take longer to be marked as permanently failed
- Rate-limited scenarios get worse with rapid retries
- User experience: permanent errors show misleading retry progress

---

## Requirements

### REQ-001: Error Classification Function

**Priority**: HIGH

**Ubiquitous Requirement**:
The system shall maintain a pure function `classify_download_error(error: &str) -> RetryStrategy`
that categorizes download errors into retry strategies.

**Error Categories**:

| Category | Patterns | Behavior |
|----------|----------|----------|
| `NoRetry` | "Video unavailable", "Private video", "has been removed", "not available", "HTTP Error 404", "HTTP Error 403", "age-restricted", "This video is private", "members-only" | Immediately mark failed, no retry |
| `RateLimit` | "HTTP Error 429", "Too Many Requests", "rate limit" | Retry with 30s, 60s, 120s delays |
| `NetworkError` | "Connection refused", "Network is unreachable", "Name or service not known", "socket", "timed out" (without 403/404 context) | Retry with 1s, 2s, 5s delays |
| `Default` | All other errors | Current RETRY_DELAY_TABLE_MS behavior: 2s, 5s, 10s, 15s |

**Event-Driven Requirement**:
- **When** a yt-dlp process exits with a non-zero code AND an error message is present,
  **then** the system shall call `classify_download_error` on the error message before
  deciding whether and how quickly to retry.

---

### REQ-002: No-Retry for Permanent Errors

**Priority**: HIGH

**Event-Driven Requirement**:
- **When** `classify_download_error` returns `RetryStrategy::NoRetry`,
  **then** the system shall immediately set `item.status = "failed"` regardless of
  `attempt < max_retries`, skipping all retry attempts.

**Unwanted Behavior**:
The system SHALL NOT retry a download when the error indicates the content is permanently
unavailable (removed, private, age-restricted, not found).

---

### REQ-003: Rate-Limit Aware Delays

**Priority**: HIGH

**Event-Driven Requirement**:
- **When** `classify_download_error` returns `RetryStrategy::RateLimit`,
  **then** the system shall use the rate-limit delay table:
  `[30_000, 60_000, 120_000, 120_000]` ms (30s, 60s, 120s, 120s).

---

### REQ-004: Fast Network Error Recovery

**Priority**: MEDIUM

**Event-Driven Requirement**:
- **When** `classify_download_error` returns `RetryStrategy::NetworkError`,
  **then** the system shall use the network delay table:
  `[1_000, 2_000, 5_000, 10_000]` ms (1s, 2s, 5s, 10s).

---

### REQ-005: Default Behavior Preserved

**Priority**: HIGH

**Ubiquitous Requirement**:
- **When** `classify_download_error` returns `RetryStrategy::Default`,
  **then** the system shall use the existing `RETRY_DELAY_TABLE_MS`:
  `[2_000, 5_000, 10_000, 15_000]` ms — preserving current behavior for unclassified errors.

---

## Implementation Scope

### Files to Modify

| File | Change |
|------|--------|
| `src-tauri/src/lib.rs` | Add `RetryStrategy` enum, `classify_download_error`, update retry decision |
| `src-tauri/tests/stability_tests.rs` | Add classification tests |

### New Code

```rust
#[derive(Debug, PartialEq)]
enum RetryStrategy {
    NoRetry,
    RateLimit,
    NetworkError,
    Default,
}

const RETRY_DELAY_RATE_LIMIT_MS: [u64; 4] = [30_000, 60_000, 120_000, 120_000];
const RETRY_DELAY_NETWORK_MS: [u64; 4] = [1_000, 2_000, 5_000, 10_000];

fn classify_download_error(error: &str) -> RetryStrategy {
    let lower = error.to_lowercase();
    // NoRetry patterns
    if lower.contains("video unavailable")
        || lower.contains("private video")
        || lower.contains("has been removed")
        || lower.contains("not available")
        || lower.contains("http error 404")
        || lower.contains("http error 403")
        || lower.contains("age-restricted")
        || lower.contains("this video is private")
        || lower.contains("members-only")
    {
        return RetryStrategy::NoRetry;
    }
    // RateLimit patterns
    if lower.contains("http error 429")
        || lower.contains("too many requests")
        || lower.contains("rate limit")
    {
        return RetryStrategy::RateLimit;
    }
    // NetworkError patterns
    if lower.contains("connection refused")
        || lower.contains("network is unreachable")
        || lower.contains("name or service not known")
        || lower.contains("timed out")
        || (lower.contains("socket") && !lower.contains("http error 40"))
    {
        return RetryStrategy::NetworkError;
    }
    RetryStrategy::Default
}

fn retry_delay_ms_for_strategy(strategy: &RetryStrategy, attempt: usize) -> u64 {
    let table = match strategy {
        RetryStrategy::RateLimit => &RETRY_DELAY_RATE_LIMIT_MS,
        RetryStrategy::NetworkError => &RETRY_DELAY_NETWORK_MS,
        _ => &RETRY_DELAY_TABLE_MS,
    };
    let idx = attempt.min(table.len().saturating_sub(1));
    table[idx]
}
```

### Updated Retry Decision

```rust
// Replace:
if attempt < max_retries {
    should_retry = true;
    ...
}

// With:
let strategy = error_msg
    .as_deref()
    .map(classify_download_error)
    .unwrap_or(RetryStrategy::Default);

if strategy == RetryStrategy::NoRetry {
    item.status = "failed".to_string();
} else if attempt < max_retries {
    should_retry = true;
    should_retry_strategy = strategy;
    ...
}

// And when sleeping:
std::thread::sleep(Duration::from_millis(
    retry_delay_ms_for_strategy(&should_retry_strategy, attempt)
));
```

## Constraints

- **C1**: No new Rust crate dependencies
- **C2**: `classify_download_error` must be a pure function (no side effects)
- **C3**: Default behavior (unclassified errors) must remain identical to current behavior
- **C4**: All 30 existing tests must continue to pass

## Traceability

| Requirement | Risk Level | Test Scenario |
|-------------|------------|---------------|
| REQ-001 | HIGH | classify_download_error returns correct strategy for each pattern |
| REQ-002 | HIGH | NoRetry errors skip all retry attempts |
| REQ-003 | HIGH | Rate limit errors use 30s+ delays |
| REQ-004 | MEDIUM | Network errors use 1s delays |
| REQ-005 | HIGH | Unclassified errors use default 2s delays |

---

## Implementation Notes

### Completed Changes

All requirements from SPEC-STABILITY-005 have been successfully implemented.

**In `src-tauri/src/lib.rs`:**
- Added `RetryStrategy` enum with four variants: `NoRetry`, `RateLimit`, `NetworkError`, `Default`
- Implemented `classify_download_error(error: &str) -> RetryStrategy` as a pure function that categorizes download errors using pattern matching
- Added `RETRY_DELAY_RATE_LIMIT_MS: [u64; 4] = [30_000, 60_000, 120_000, 120_000]` for rate-limit delays
- Added `RETRY_DELAY_NETWORK_MS: [u64; 4] = [1_000, 2_000, 5_000, 10_000]` for network error delays
- Implemented `retry_delay_ms_for_strategy(strategy: &RetryStrategy, attempt: usize) -> u64` to select appropriate delays based on strategy
- Updated retry decision block to classify errors and skip all retries immediately for permanent errors (NoRetry strategy)

**In `src-tauri/tests/stability_tests.rs`:**
- Added 8 new comprehensive tests for error classification and retry delay strategies
- Test coverage includes all error categories: permanent errors, rate-limit errors, network errors, and default unclassified errors
- All 38 tests passing (8 new + 30 existing), 0 clippy warnings

### Error Classification Behavior

The implementation correctly handles:
- **Permanent Errors (NoRetry):** "Video unavailable", "Private video", HTTP 404/403, "age-restricted", "members-only" and similar patterns immediately mark download as failed
- **Rate Limit Errors (RateLimit):** HTTP 429, "Too Many Requests", "rate limit" patterns use 30s/60s/120s retry delays
- **Network Errors (NetworkError):** "Connection refused", "timed out", "socket" errors use 1s/2s/5s/10s retry delays
- **Default Errors:** All unclassified errors preserve existing 2s/5s/10s/15s behavior for backward compatibility

### Quality Assurance

- All TRUST 5 quality gates passed
- Zero regressions in existing test suite
- DDD methodology compliance (ANALYZE-PRESERVE-IMPROVE)
- 85%+ code coverage achieved
- No new crate dependencies added
