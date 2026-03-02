use crate::download::{kill_active_child_unchecked, start_worker_if_needed, SharedRuntime};
use crate::file_ops::{queue_file_path, write_atomic};
use crate::metadata::DownloadMode;
use crate::settings::{AppState, SharedState};
use crate::types::CommandResult;
use crate::utils::{normalize_youtube_video_url, sanitize_file_name};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

const MAX_LOG_LINES_PER_JOB: usize = 120;

// ============================================================================
// Domain types
// ============================================================================

/// Represents a single download job in the queue with its current state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    pub url: String,
    pub mode: DownloadMode,
    pub quality_id: String,
    pub status: String,
    pub progress_percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub retry_count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_log: Option<Vec<String>>,
}

/// A snapshot of all queue items emitted to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct QueueSnapshot {
    pub items: Vec<QueueItem>,
}

/// Result of a duplicate-URL check for a given mode and quality.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCheckResult {
    pub is_duplicate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_output_path: Option<String>,
}

/// Input parameters for the `check_duplicate` command.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckDuplicateInput {
    pub url: String,
    pub mode: DownloadMode,
    pub quality_id: String,
}

/// Input parameters for the `enqueue_job` command.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueInput {
    pub url: String,
    pub title: Option<String>,
    pub thumbnail_url: Option<String>,
    pub mode: DownloadMode,
    pub quality_id: String,
    pub force_duplicate: bool,
}

// ============================================================================
// Queue snapshot utilities
// ============================================================================

/// Builds a QueueSnapshot from the current AppState.
// @MX:ANCHOR: [AUTO] Used by all queue commands and worker thread to emit state updates. fan_in=10.
// @MX:REASON: [AUTO] Central read path: enqueue_job, pause_job, resume_job, cancel_job, clear_terminal_jobs, delete_file, get_queue_snapshot, and worker all call this.
pub fn queue_snapshot(state: &AppState) -> QueueSnapshot {
    QueueSnapshot {
        items: state.queue.clone(),
    }
}

/// Emits a queue-updated event from the current AppState.
pub fn emit_queue_updated(app: &AppHandle, state: &AppState) {
    let _ = app.emit("queue-updated", queue_snapshot(state));
}

/// Emits a queue-updated event from an already-built QueueSnapshot.
pub fn emit_queue_updated_snapshot(app: &AppHandle, snapshot: QueueSnapshot) {
    let _ = app.emit("queue-updated", snapshot);
}

// ============================================================================
// Persistence helpers
// ============================================================================

/// Persists the current queue to disk atomically, writing a backup after success.
// @MX:ANCHOR: [AUTO] All queue state persisted through this function. fan_in=8.
// @MX:REASON: [AUTO] High fan_in: cancel_job, pause_job, resume_job, enqueue_job, download worker, clear_terminal_jobs use this path.
pub fn persist_queue(app: &AppHandle, state: &AppState) {
    let path = queue_file_path(app);
    if let Ok(serialized) = serde_json::to_string_pretty(&state.queue) {
        if write_atomic(&path, &serialized).is_ok() {
            let bak_path = PathBuf::from(format!("{}.bak", path.display()));
            let _ = write_atomic(&bak_path, &serialized);
        }
    }
}

// ============================================================================
// Queue utility functions
// ============================================================================

/// Returns the expected output extension for a given DownloadMode.
pub fn expected_extension(mode: &DownloadMode) -> &'static str {
    match mode {
        DownloadMode::Audio => "mp3",
        DownloadMode::Video => "mp4",
    }
}

/// Builds a unique output path for a new download, avoiding collisions.
pub fn build_unique_output_path(state: &AppState, title: &str, mode: &DownloadMode) -> PathBuf {
    let ext = expected_extension(mode);
    let base = sanitize_file_name(title);
    let root = PathBuf::from(&state.settings.download_dir);
    let mut suffix: i32 = 0;

    loop {
        let suffix_label = if suffix == 0 {
            "".to_string()
        } else {
            format!(" ({suffix})")
        };
        let file_name = format!("{base}{suffix_label}.{ext}");
        let candidate = root.join(file_name);
        let exists_on_disk = candidate.exists();
        let exists_in_queue = state.queue.iter().any(|item| {
            item.output_path
                .as_ref()
                .map(|p| *p == candidate)
                .unwrap_or(false)
                && item.status != "failed"
                && item.status != "canceled"
        });
        if !exists_on_disk && !exists_in_queue {
            return candidate;
        }
        suffix += 1;
    }
}

/// Selects the yt-dlp format expression for a download.
pub fn select_format_expression(mode: &DownloadMode, quality_id: &str) -> String {
    match mode {
        DownloadMode::Audio => quality_id.to_string(),
        DownloadMode::Video => {
            if quality_id.contains('+') {
                quality_id.to_string()
            } else {
                format!("{quality_id}+bestaudio/best[acodec!=none]/best")
            }
        }
    }
}

/// Appends a log line to a QueueItem's download_log, enforcing MAX_LOG_LINES_PER_JOB.
/// Returns true if the line was actually appended (deduplication: skips if identical to last).
pub fn append_download_log(item: &mut QueueItem, line: &str) -> bool {
    let log = item.download_log.get_or_insert_with(Vec::new);
    if log.last().map(|last| last == line).unwrap_or(false) {
        return false;
    }
    log.push(line.to_string());
    if log.len() > MAX_LOG_LINES_PER_JOB {
        let overflow = log.len() - MAX_LOG_LINES_PER_JOB;
        log.drain(0..overflow);
    }
    true
}

/// Scans the download directory for `.incomplete` marker files at startup.
/// For each marker: finds matching queue item by output_path, marks it failed,
/// then removes the marker regardless of whether a matching item was found.
pub fn scan_incomplete_markers(app: &AppHandle, state: &mut AppState) {
    let download_dir = PathBuf::from(&state.settings.download_dir);
    let entries = match fs::read_dir(&download_dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if !name.ends_with(".incomplete") {
            continue;
        }
        // Derive the intended destination path by stripping ".incomplete"
        let dest_str = name.trim_end_matches(".incomplete");
        let dest_path = download_dir.join(dest_str);
        let dest_str_full = dest_path.to_string_lossy().to_string();

        // Find a matching queue item by output_path
        if let Some(item) = state
            .queue
            .iter_mut()
            .find(|i| i.output_path.as_deref() == Some(&dest_str_full))
        {
            item.status = "failed".to_string();
            item.error_message =
                Some("Transfer incomplete - file may be corrupted. Please retry.".to_string());
        }
        // Remove the .incomplete marker regardless of match
        let _ = fs::remove_file(&path);
    }
    // Emit event to surface the updated queue state if app handle is available
    let _ = app.emit("queue-updated", serde_json::json!({}));
}

// ============================================================================
// Tauri commands
// ============================================================================

/// Checks whether a URL with the given mode and quality is already in the active queue.
#[tauri::command]
pub async fn check_duplicate(
    state: State<'_, SharedState>,
    input: CheckDuplicateInput,
) -> CommandResult<DuplicateCheckResult> {
    let normalized_url = normalize_youtube_video_url(&input.url);
    let state = state
        .0
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    let duplicate = state.queue.iter().find(|item| {
        item.url == normalized_url
            && item.mode == input.mode
            && item.quality_id == input.quality_id
            && item.status != "failed"
            && item.status != "canceled"
    });
    Ok(DuplicateCheckResult {
        is_duplicate: duplicate.is_some(),
        existing_output_path: duplicate.and_then(|item| item.output_path.clone()),
    })
}

/// Adds a new download job to the queue and starts the worker if needed.
#[tauri::command]
pub async fn enqueue_job(
    app: AppHandle,
    state: State<'_, SharedState>,
    runtime: State<'_, SharedRuntime>,
    input: EnqueueInput,
) -> CommandResult<Value> {
    let normalized_url = normalize_youtube_video_url(&input.url);

    let mut locked = state
        .0
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    if !input.force_duplicate {
        let duplicate = locked.queue.iter().find(|item| {
            item.url == normalized_url
                && item.mode == input.mode
                && item.quality_id == input.quality_id
                && item.status != "failed"
                && item.status != "canceled"
        });
        if duplicate.is_some() {
            return Err("중복 다운로드가 감지되었습니다.".to_string());
        }
    }

    let id = Uuid::new_v4().to_string();
    locked.queue.push(QueueItem {
        id: id.clone(),
        title: input.title.unwrap_or_else(|| normalized_url.clone()),
        thumbnail_url: input.thumbnail_url,
        url: normalized_url,
        mode: input.mode,
        quality_id: input.quality_id,
        status: "queued".to_string(),
        progress_percent: 0.0,
        speed_text: None,
        eta_text: None,
        output_path: None,
        error_message: None,
        retry_count: 0,
        download_log: Some(Vec::new()),
    });
    emit_queue_updated(&app, &locked);
    persist_queue(&app, &locked);
    drop(locked);

    start_worker_if_needed(app.clone(), state.0.clone(), runtime.0.clone());
    Ok(serde_json::json!({ "jobId": id }))
}

/// Pauses an active or queued job and kills the running subprocess.
#[tauri::command]
pub async fn pause_job(
    app: AppHandle,
    state: State<'_, SharedState>,
    runtime: State<'_, SharedRuntime>,
    id: String,
) -> CommandResult<QueueSnapshot> {
    let mut state = state
        .0
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    if let Some(item) = state.queue.iter_mut().find(|item| item.id == id) {
        item.status = "paused".to_string();
        item.speed_text = None;
        item.eta_text = None;
    }
    let snapshot = queue_snapshot(&state);
    emit_queue_updated(&app, &state);
    persist_queue(&app, &state);

    let runtime = runtime.0.clone();
    std::thread::spawn(move || {
        kill_active_child_unchecked(&runtime);
    });

    Ok(snapshot)
}

/// Resumes a paused or failed job by re-queuing it and starting the worker.
#[tauri::command]
pub async fn resume_job(
    app: AppHandle,
    state: State<'_, SharedState>,
    runtime: State<'_, SharedRuntime>,
    id: String,
) -> CommandResult<QueueSnapshot> {
    let shared_state = state.0.clone();
    let mut locked = shared_state
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    if let Some(item) = locked.queue.iter_mut().find(|item| item.id == id) {
        item.status = "queued".to_string();
        item.error_message = None;
    }
    let snapshot = queue_snapshot(&locked);
    emit_queue_updated(&app, &locked);
    persist_queue(&app, &locked);
    drop(locked);

    start_worker_if_needed(app.clone(), shared_state, runtime.0.clone());
    Ok(snapshot)
}

/// Cancels a job by marking it canceled and killing the running subprocess.
#[tauri::command]
pub async fn cancel_job(
    app: AppHandle,
    state: State<'_, SharedState>,
    runtime: State<'_, SharedRuntime>,
    id: String,
) -> CommandResult<QueueSnapshot> {
    let mut state = state
        .0
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    if let Some(item) = state.queue.iter_mut().find(|item| item.id == id) {
        item.status = "canceled".to_string();
        item.error_message = Some("사용자 취소".to_string());
    }
    let snapshot = queue_snapshot(&state);
    emit_queue_updated(&app, &state);
    persist_queue(&app, &state);
    drop(state);

    let runtime = runtime.0.clone();
    std::thread::spawn(move || {
        kill_active_child_unchecked(&runtime);
    });

    Ok(snapshot)
}

/// Removes all completed, failed, and canceled jobs from the queue.
#[tauri::command]
pub async fn clear_terminal_jobs(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> CommandResult<QueueSnapshot> {
    let mut state = state
        .0
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    state.queue.retain(|item| {
        item.status != "completed" && item.status != "failed" && item.status != "canceled"
    });
    let snapshot = queue_snapshot(&state);
    emit_queue_updated(&app, &state);
    persist_queue(&app, &state);
    Ok(snapshot)
}

/// Returns a snapshot of the current queue state.
#[tauri::command]
pub async fn get_queue_snapshot(state: State<'_, SharedState>) -> CommandResult<QueueSnapshot> {
    let state = state
        .0
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    Ok(queue_snapshot(&state))
}

/// Removes a queue entry by output file path and persists the updated queue.
#[tauri::command]
pub async fn delete_file(
    app: AppHandle,
    state: State<'_, SharedState>,
    path: String,
) -> CommandResult<QueueSnapshot> {
    let mut state = state
        .0
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    state.queue.retain(|item| {
        item.output_path
            .as_ref()
            .map(|p| p != &path)
            .unwrap_or(true)
    });
    let snapshot = queue_snapshot(&state);
    emit_queue_updated(&app, &state);
    persist_queue(&app, &state);
    Ok(snapshot)
}
