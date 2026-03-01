---
id: SPEC-STABILITY-002
document: acceptance
version: 1.0.0
spec-ref: SPEC-STABILITY-002/spec.md
---

# Acceptance Criteria: SPEC-STABILITY-002 - Data Integrity & Persistence

## TC-001: Queue File Corruption Recovery (REQ-001)

### TC-001-A: Happy Path - Valid Queue Loads Normally (PRESERVE)

**Given** a valid `queue_state.json` file exists with 3 QueueItem entries
**When** the application starts and `load_queue_with_recovery` is called
**Then** all 3 items are loaded into `state.queue`
**And** no corruption events are emitted
**And** a backup file `queue_state.json.bak` is created (or updated) with the same content

---

### TC-001-B: Happy Path - Downloading Items Reset to Queued (PRESERVE)

**Given** `queue_state.json` contains an item with `status: "downloading"`
**When** the application starts and `load_queue_with_recovery` is called
**Then** that item's status is reset to `"queued"`
**And** no data is lost from the item

---

### TC-001-C: Happy Path - Missing Queue File (PRESERVE)

**Given** `queue_state.json` does not exist
**When** `load_queue_with_recovery` is called
**Then** `state.queue` is initialized as an empty Vec
**And** no error events are emitted
**And** no backup file is created

---

### TC-001-D: Corruption Recovery - Backup Available (IMPROVE)

**Given** `queue_state.json` contains malformed JSON (e.g., `{"broken":`)
**And** `queue_state.json.bak` exists with valid JSON containing 2 QueueItem entries
**When** the application starts and `load_queue_with_recovery` is called
**Then** the 2 items from the backup are loaded into `state.queue`
**And** a `queue-corruption-recovered` event is emitted with `backup_item_count: 2`
**And** `state.queue` contains exactly 2 items

---

### TC-001-E: Corruption Recovery - No Backup Available (IMPROVE)

**Given** `queue_state.json` contains malformed JSON
**And** `queue_state.json.bak` does not exist
**When** the application starts and `load_queue_with_recovery` is called
**Then** `state.queue` is initialized as an empty Vec
**And** a `queue-corruption-unrecoverable` event is emitted with a non-empty `error` field
**And** the application continues to start normally (no panic)

---

### TC-001-F: Corruption Recovery - Both Files Corrupted (IMPROVE)

**Given** `queue_state.json` contains malformed JSON
**And** `queue_state.json.bak` also contains malformed JSON
**When** the application starts and `load_queue_with_recovery` is called
**Then** `state.queue` is initialized as an empty Vec
**And** a `queue-corruption-unrecoverable` event is emitted
**And** the application continues to start normally

---

### TC-001-G: Atomic Backup Write During Persist (IMPROVE)

**Given** the application has a valid queue state in memory
**When** `persist_queue` is called
**Then** `queue_state.json` is written atomically (via temp file + rename)
**And** `queue_state.json.bak` is subsequently updated with the same content
**And** there is no time window where `queue_state.json` is empty or truncated

---

## TC-002: Atomic Download Completion (REQ-002)

### TC-002-A: Same-Filesystem Move (PRESERVE)

**Given** a download has completed successfully
**And** the temp directory and final output directory are on the same filesystem
**When** `move_file_atomic` is called
**Then** `fs::rename` is used (no copy performed)
**And** no `.incomplete` marker file is created
**And** the item status is set to `"completed"` after successful rename
**And** the original temp file no longer exists

---

### TC-002-B: Cross-Device Move - Success (IMPROVE)

**Given** a download has completed successfully
**And** the temp directory and final output directory are on different filesystems
**When** `move_file_atomic` is called
**Then** a `{destination}.incomplete` marker file is created before copying begins
**And** the file is copied from source to destination
**And** the destination file size matches the source file size
**And** the `.incomplete` marker file is removed after successful copy
**And** the source temp file is removed
**And** the item status is set to `"completed"`

---

### TC-002-C: Cross-Device Move - Copy Failure (IMPROVE)

**Given** a download has completed successfully
**And** the cross-device copy operation fails midway (e.g., disk full)
**When** `move_file_atomic` is called
**Then** the `{destination}.incomplete` marker file remains on disk
**And** `move_file_atomic` returns `Err`
**And** the item status is set to `"failed"` with a descriptive error message
**And** the item is NOT marked as `"completed"`

---

### TC-002-D: Incomplete Marker Detection at Startup (IMPROVE)

**Given** a `{output_path}.incomplete` marker file exists on disk
**And** a corresponding queue item with that `output_path` exists in the queue
**When** `scan_incomplete_markers` runs during application startup
**Then** the corresponding queue item status is set to `"failed"`
**And** the item's `error_message` contains text indicating incomplete transfer
**And** the `.incomplete` marker file is removed from disk
**And** the user can retry the download

---

### TC-002-E: Incomplete Marker - No Matching Queue Item (IMPROVE)

**Given** a `{output_path}.incomplete` marker file exists on disk
**And** no queue item has that `output_path`
**When** `scan_incomplete_markers` runs during application startup
**Then** the `.incomplete` marker file is removed from disk
**And** no error is emitted (orphaned marker is silently cleaned up)

---

### TC-002-F: File Size Verification After Copy (IMPROVE)

**Given** a cross-device copy completes
**But** the destination file size does not match the source file size
**When** the size verification step runs
**Then** `move_file_atomic` returns `Err` with a size mismatch error message
**And** the `.incomplete` marker remains on disk for startup detection
**And** the destination partial file is removed

---

## TC-003: Settings Corruption Recovery (REQ-003)

### TC-003-A: Happy Path - Valid Settings Load Normally (PRESERVE)

**Given** a valid `settings.json` exists with `download_dir`, `max_retries: 3`, and `language: "en"`
**When** the application starts and `load_settings_with_recovery` is called
**Then** `state.settings.max_retries` equals 3
**And** `state.settings.language` equals `"en"`
**And** no corruption events are emitted
**And** `settings.json.bak` is created with the same content

---

### TC-003-B: Happy Path - Missing Settings File (PRESERVE)

**Given** `settings.json` does not exist
**When** `load_settings_with_recovery` is called
**Then** application defaults are used for all settings
**And** no error events are emitted

---

### TC-003-C: Corruption Recovery - Backup Available (IMPROVE)

**Given** `settings.json` contains malformed JSON
**And** `settings.json.bak` exists with valid JSON containing `max_retries: 5`
**When** the application starts and `load_settings_with_recovery` is called
**Then** settings are restored from backup (`state.settings.max_retries` equals 5)
**And** a `settings-corruption-recovered` event is emitted
**And** the application continues to start normally

---

### TC-003-D: Corruption Recovery - No Backup Available (IMPROVE)

**Given** `settings.json` contains malformed JSON
**And** `settings.json.bak` does not exist
**When** the application starts and `load_settings_with_recovery` is called
**Then** application defaults are used for all settings
**And** a `settings-corruption-unrecoverable` event is emitted with a non-empty `error` field
**And** the application continues to start normally (no panic)

---

### TC-003-E: Atomic Backup Write During Settings Persist (IMPROVE)

**Given** the user saves new settings
**When** the settings persistence function is called
**Then** `settings.json` is written atomically (via temp file + rename)
**And** `settings.json.bak` is subsequently updated
**And** there is no time window where `settings.json` is empty or truncated

---

## Quality Gates

### Definition of Done

- [ ] All TC-001 A-G scenarios pass
- [ ] All TC-002 A-F scenarios pass
- [ ] All TC-003 A-E scenarios pass
- [ ] No existing tests regress (characterization tests pass)
- [ ] No `unwrap()` added in new production code (per Rust rules)
- [ ] No new external crate dependencies introduced
- [ ] All new public functions have `///` doc comments
- [ ] `cargo clippy -- -D warnings` passes with no new warnings
- [ ] `cargo test` passes

### Test Classification

| Test | Type | Framework |
|------|------|-----------|
| TC-001-A through TC-001-C | Characterization (PRESERVE) | `cargo test` |
| TC-001-D through TC-001-G | Specification (IMPROVE) | `cargo test` |
| TC-002-A | Characterization (PRESERVE) | `cargo test` |
| TC-002-B through TC-002-F | Specification (IMPROVE) | `cargo test` |
| TC-003-A through TC-003-B | Characterization (PRESERVE) | `cargo test` |
| TC-003-C through TC-003-E | Specification (IMPROVE) | `cargo test` |

### Edge Cases to Verify

1. Concurrent `persist_queue` calls do not corrupt backup (mutex is held during write)
2. Backup file write failure does not cause `persist_queue` to return error (backup is best-effort)
3. `scan_incomplete_markers` called when download directory does not yet exist (graceful skip)
4. Settings `max_retries` from backup is still clamped to `[0, 10]` after recovery
5. Queue items recovered from backup still have `download_log` initialized to empty Vec if `None`
