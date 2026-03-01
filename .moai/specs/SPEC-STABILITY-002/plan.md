---
id: SPEC-STABILITY-002
document: plan
version: 1.0.0
spec-ref: SPEC-STABILITY-002/spec.md
---

# Implementation Plan: SPEC-STABILITY-002 - Data Integrity & Persistence

## DDD Approach: ANALYZE-PRESERVE-IMPROVE

This plan follows the DDD methodology:
- **ANALYZE**: Understand current behavior and capture characterization tests
- **PRESERVE**: Ensure existing behavior (happy path) remains unchanged
- **IMPROVE**: Add recovery and notification capabilities

---

## Architecture Analysis

### Affected Functions

| Function | Location | Lines | Change Type |
|----------|----------|-------|-------------|
| `load_settings` | `src-tauri/src/lib.rs` | 883-900 | IMPROVE: Add backup/recovery/notification |
| `load_queue` | `src-tauri/src/lib.rs` | 902-919 | IMPROVE: Add backup/recovery/notification |
| `move_file_with_fallback` | `src-tauri/src/lib.rs` | 930-948 | IMPROVE: Add incomplete marker pattern |
| `persist_queue` | `src-tauri/src/lib.rs` | TBD | IMPROVE: Atomic write with backup creation |
| `persist_settings` (new/existing) | `src-tauri/src/lib.rs` | TBD | IMPROVE: Atomic write with backup creation |
| App startup (setup) | `src-tauri/src/lib.rs` | TBD | IMPROVE: Incomplete marker detection |

### New Helper Functions (to be introduced)

| Function | Purpose | REQ |
|----------|---------|-----|
| `write_atomic(path, content)` | Write via temp file + rename to prevent partial writes | REQ-001, REQ-003 |
| `load_queue_with_recovery(app, state)` | Replaces `load_queue`, adds backup/recovery | REQ-001 |
| `load_settings_with_recovery(app, state)` | Replaces `load_settings`, adds backup/recovery | REQ-003 |
| `move_file_atomic(source, dest)` | Cross-device move with incomplete marker | REQ-002 |
| `scan_incomplete_markers(app, state)` | Startup scan for `.incomplete` files | REQ-002 |

### Event Definitions (Frontend Integration)

| Event Name | Payload | Trigger |
|-----------|---------|---------|
| `queue-corruption-recovered` | `{ backup_item_count: usize, message: String }` | Queue backup recovery succeeded |
| `queue-corruption-unrecoverable` | `{ error: String }` | Both queue and backup corrupted |
| `settings-corruption-recovered` | `{ message: String }` | Settings backup recovery succeeded |
| `settings-corruption-unrecoverable` | `{ error: String }` | Both settings and backup corrupted |

---

## Milestone 1 (Primary Goal): Atomic Persistence Infrastructure

**Goal**: Introduce `write_atomic` helper and integrate it into all JSON persistence paths.

### Tasks

**Task 1.1 - ANALYZE**: Locate all `persist_queue` and settings write call sites.

- Read `persist_queue` function implementation
- Read settings persistence implementation (if exists)
- Identify all callers via `Grep`
- Capture characterization test: "given valid queue, persist_queue writes valid JSON"

**Task 1.2 - PRESERVE**: Write characterization tests for existing happy-path persistence.

- Test: `persist_queue` writes all queue items to `queue_state.json`
- Test: File content is valid `Vec<QueueItem>` JSON after `persist_queue`
- Test: `load_queue` correctly reads back what `persist_queue` wrote

**Task 1.3 - IMPROVE**: Implement `write_atomic` function.

```rust
// Proposed signature - implementation deferred to run phase
fn write_atomic(path: &Path, content: &str) -> Result<(), String>
// 1. Write content to `{path}.tmp`
// 2. fs::rename `{path}.tmp` to `path` (atomic on POSIX, near-atomic on Windows)
// 3. Return Ok(()) or propagate error
```

**Task 1.4 - IMPROVE**: Update `persist_queue` to use `write_atomic` + create backup.

- After successful atomic write, copy `queue_state.json` to `queue_state.json.bak`
- Use `write_atomic` for the backup write as well

**Acceptance**: Task 1.2 tests still pass after Task 1.3 and 1.4 changes.

---

## Milestone 2 (Primary Goal): Queue Corruption Recovery (REQ-001)

**Goal**: Replace silent discard in `load_queue` with backup-aware recovery and user notification.

### Tasks

**Task 2.1 - ANALYZE**: Trace all callers of `load_queue`.

- Identify where `load_queue` is called during app startup
- Verify event emission capability at that point in startup sequence

**Task 2.2 - PRESERVE**: Add characterization test for current behavior.

- Test: Given missing `queue_state.json`, queue initializes as empty Vec
- Test: Given valid `queue_state.json`, queue loads all items correctly
- Test: Given `downloading` status items, they are reset to `queued` on load

**Task 2.3 - IMPROVE**: Implement `load_queue_with_recovery`.

Recovery logic (pseudocode for specification):
```
1. Try read queue_state.json
   - If read fails (file missing): return empty queue (existing behavior preserved)
   - If read succeeds: try parse JSON
     a. If parse OK: load queue, update backup → DONE (primary path)
     b. If parse fails: corruption detected → go to step 2

2. Corruption detected:
   a. Try read queue_state.json.bak
   b. If backup valid: restore from backup, emit queue-corruption-recovered
   c. If backup missing/invalid: emit queue-corruption-unrecoverable, return empty queue
```

**Task 2.4 - IMPROVE**: Replace `load_queue` call site with `load_queue_with_recovery`.

**Acceptance**: Task 2.2 characterization tests still pass; new recovery scenarios match acceptance.md#TC-001.

---

## Milestone 3 (Primary Goal): Atomic Download Completion (REQ-002)

**Goal**: Make cross-device file moves safe against power failure.

### Tasks

**Task 3.1 - ANALYZE**: Understand `move_file_with_fallback` and its call site.

- Read `move_file_with_fallback` (lines 930-948)
- Read `resolve_downloaded_file_path` (lines 951+)
- Read the download completion block (lines 1507-1522)
- Identify: what is the temp directory path structure?

**Task 3.2 - PRESERVE**: Characterization tests for existing behavior.

- Test: Same-filesystem move uses `fs::rename` (verify no `.incomplete` on same FS)
- Test: Cross-device move copies content correctly and removes source
- Test: Move failure returns `Err` and item is marked `failed`

**Task 3.3 - IMPROVE**: Implement `move_file_atomic` with incomplete marker.

Cross-device move sequence:
```
1. Write `{destination}.incomplete` marker (empty file or JSON with metadata)
2. fs::copy(source, destination)
3. Verify: destination file size == source file size
4. fs::remove_file(`{destination}.incomplete`)
5. fs::remove_file(source)
6. Return Ok(())
```

If any step 2-5 fails: return `Err`, leave `.incomplete` marker for startup detection.

**Task 3.4 - IMPROVE**: Implement `scan_incomplete_markers` for startup.

- Scan configured download directories for `*.incomplete` marker files
- For each found: locate corresponding queue item by output_path
- Mark item status as `failed` with message: "Transfer incomplete - file may be corrupted. Please retry."
- Remove the `.incomplete` marker file

**Task 3.5 - IMPROVE**: Integrate `scan_incomplete_markers` into app startup before `load_queue`.

**Acceptance**: Task 3.2 characterization tests still pass; new scenarios match acceptance.md#TC-002.

---

## Milestone 4 (Secondary Goal): Settings Corruption Recovery (REQ-003)

**Goal**: Replace silent settings loss with backup-aware recovery and user notification.

### Tasks

**Task 4.1 - ANALYZE**: Verify settings persistence patterns.

- Locate where settings are written to `settings.json`
- Identify if atomic write is already used

**Task 4.2 - PRESERVE**: Characterization tests for current behavior.

- Test: Given valid `settings.json`, settings load correctly
- Test: Given missing `settings.json`, defaults are used (existing behavior)
- Test: Given malformed `settings.json`, defaults are used (current silent behavior - to be changed)

**Task 4.3 - IMPROVE**: Implement `load_settings_with_recovery`.

Same pattern as `load_queue_with_recovery` but for settings:
```
1. Try read settings.json
   - If missing: use defaults (preserved behavior)
   - If valid: apply settings, update backup
   - If malformed: try backup recovery

2. Backup recovery:
   a. Try settings.json.bak
   b. If valid: restore, emit settings-corruption-recovered
   c. If invalid: use defaults, emit settings-corruption-unrecoverable
```

**Task 4.4 - IMPROVE**: Update settings persistence to use `write_atomic` + backup.

**Acceptance**: Task 4.2 characterization tests still pass; new scenarios match acceptance.md#TC-003.

---

## Milestone 5 (Optional Goal): Frontend Notification UI

**Goal**: Display user-visible toasts/alerts when corruption events are received.

### Tasks

**Task 5.1**: Implement event listeners in frontend for all 4 corruption events.
**Task 5.2**: Display non-blocking toast notifications with details:
  - Recovered: "Your [queue/settings] had an issue but was restored from backup."
  - Unrecoverable: "Your [queue/settings] could not be recovered. Starting fresh."

---

## Technical Approach

### write_atomic Pattern (Cross-Platform)

The atomic write uses a temp file + rename pattern:
- On POSIX systems: `rename()` is atomic (POSIX guarantee)
- On Windows: `MoveFileEx` with `MOVEFILE_REPLACE_EXISTING` provides near-atomic behavior
- Temp file written in same directory as target (same filesystem = same rename domain)

### Incomplete Marker Format

The `.incomplete` marker file stores JSON metadata:
```json
{
  "started_at": "<ISO8601 timestamp>",
  "source_size": <bytes>,
  "queue_item_id": "<uuid>"
}
```
This enables richer recovery information at startup.

### Backup File Naming Convention

| Original File | Backup File |
|--------------|-------------|
| `queue_state.json` | `queue_state.json.bak` |
| `settings.json` | `settings.json.bak` |

---

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Backup write fails after primary write succeeds | Low | Low | Log warning; backup is best-effort |
| `scan_incomplete_markers` scans wrong directory | Medium | Medium | Use `queue_file_path(app).parent()` for app data dir |
| Windows rename semantics differ from POSIX | Medium | Medium | Test explicitly on Windows in acceptance tests |
| Frontend events emitted before window is ready | Medium | High | Buffer events or emit after window ready signal |
| Incomplete marker orphaned if queue item deleted | Low | Low | Scan at startup removes orphaned markers regardless |

## Implementation Order

1. Milestone 1 (atomic persistence) - enables safe backup creation for M2 and M4
2. Milestone 3 (atomic download) - highest user impact, independent of M1
3. Milestone 2 (queue recovery) - depends on M1 for backup infrastructure
4. Milestone 4 (settings recovery) - depends on M1, lower impact than M2/M3
5. Milestone 5 (frontend UI) - cosmetic, deferred to after core logic is verified
