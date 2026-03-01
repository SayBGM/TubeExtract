mod dependencies;
mod diagnostics;
mod file_ops;
mod state;
mod types;
mod utils;

use crate::dependencies::{
    default_dependency_status, emit_dependency_status, start_dependency_bootstrap_if_needed,
    wait_for_dependencies, DependencyBootstrapStatus, DependencyRuntimeState,
    SharedDependencyState,
};
use crate::diagnostics::{
    check_update, get_storage_stats, open_external_url, open_folder, run_diagnostics,
};
use crate::file_ops::{
    configure_hidden_process, managed_path_env, move_file_atomic, normalize_download_dir,
    queue_file_path, remove_directory_safe, resolve_downloaded_file_path, resolve_executable,
    run_command_capture, settings_file_path, temp_downloads_root_dir, temp_job_dir_path,
    write_atomic,
};
use crate::state::lock_or_recover;
use crate::types::CommandResult;
use crate::utils::{
    normalize_youtube_video_url, parse_eta, parse_progress_percent, parse_speed, sanitize_file_name,
};
use dirs::download_dir;
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, TryLockError};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

const MAX_LOG_LINES_PER_JOB: usize = 120;
const RETRY_DELAY_TABLE_MS: [u64; 4] = [2000, 5000, 10000, 15000];
const RETRY_DELAY_RATE_LIMIT_MS: [u64; 4] = [30_000, 60_000, 120_000, 120_000];
const RETRY_DELAY_NETWORK_MS: [u64; 4] = [1_000, 2_000, 5_000, 10_000];
const ANALYZE_TIMEOUT_MS: u64 = 15_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum DownloadMode {
    Video,
    Audio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QualityOption {
    id: String,
    label: String,
    ext: String,
    #[serde(rename = "type")]
    mode: DownloadMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnalysisResult {
    #[serde(rename = "sourceUrl")]
    source_url: String,
    title: String,
    channel: String,
    #[serde(rename = "durationSec")]
    duration_sec: i64,
    #[serde(rename = "thumbnailUrl")]
    thumbnail_url: String,
    #[serde(rename = "videoOptions")]
    video_options: Vec<QualityOption>,
    #[serde(rename = "audioOptions")]
    audio_options: Vec<QualityOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueItem {
    id: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbnail_url: Option<String>,
    url: String,
    mode: DownloadMode,
    quality_id: String,
    status: String,
    progress_percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    speed_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eta_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_message: Option<String>,
    retry_count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    download_log: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
struct QueueSnapshot {
    items: Vec<QueueItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    download_dir: String,
    max_retries: i32,
    language: String,
    max_concurrent_downloads: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateCheckResult {
    is_duplicate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    existing_output_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckDuplicateInput {
    url: String,
    mode: DownloadMode,
    quality_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnqueueInput {
    url: String,
    title: Option<String>,
    thumbnail_url: Option<String>,
    mode: DownloadMode,
    quality_id: String,
    force_duplicate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedSettings {
    download_dir: Option<String>,
    max_retries: Option<i32>,
    language: Option<String>,
    max_concurrent_downloads: Option<i32>,
}

#[derive(Debug, Clone)]
struct AppState {
    queue: Vec<QueueItem>,
    settings: AppSettings,
    active_worker_count: usize,
}

#[derive(Clone)]
struct SharedState(Arc<Mutex<AppState>>);

#[derive(Clone)]
struct ActiveProcess {
    #[allow(dead_code)]
    job_id: String,
    child: Arc<Mutex<Child>>,
}

#[derive(Default)]
struct RuntimeState {
    active_processes: std::collections::HashMap<String, ActiveProcess>,
    // Shutdown senders for graceful worker thread termination.
    // One entry per active worker thread.
    shutdown_txs: Vec<std::sync::mpsc::Sender<()>>,
    // Handles for worker threads.
    worker_handles: Vec<std::thread::JoinHandle<()>>,
}

#[derive(Clone)]
struct SharedRuntime(Arc<Mutex<RuntimeState>>);

fn default_settings() -> AppSettings {
    AppSettings {
        download_dir: download_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .to_string_lossy()
            .to_string(),
        max_retries: 3,
        language: "ko".to_string(),
        max_concurrent_downloads: 2,
    }
}

fn queue_snapshot(state: &AppState) -> QueueSnapshot {
    QueueSnapshot {
        items: state.queue.clone(),
    }
}

fn emit_queue_updated(app: &AppHandle, state: &AppState) {
    let _ = app.emit("queue-updated", queue_snapshot(state));
}

fn emit_queue_updated_snapshot(app: &AppHandle, snapshot: QueueSnapshot) {
    let _ = app.emit("queue-updated", snapshot);
}

/// Persists the current queue to disk atomically, writing a backup after success.
// @MX:ANCHOR: [AUTO] All queue state persisted through this function. fan_in=8.
// @MX:REASON: [AUTO] High fan_in: cancel_job, pause_job, resume_job, enqueue_job, download worker, clear_terminal_jobs use this path.
fn persist_queue(app: &AppHandle, state: &AppState) {
    let path = queue_file_path(app);
    if let Ok(serialized) = serde_json::to_string_pretty(&state.queue) {
        if write_atomic(&path, &serialized).is_ok() {
            // Write backup after successful primary write
            let bak_path = PathBuf::from(format!("{}.bak", path.display()));
            let _ = write_atomic(&bak_path, &serialized);
        }
    }
}

/// Persists settings to disk atomically, writing a backup after success.
fn persist_settings(app: &AppHandle, settings: &AppSettings) {
    let path = settings_file_path(app);
    if let Ok(serialized) = serde_json::to_string_pretty(settings) {
        if write_atomic(&path, &serialized).is_ok() {
            // Write backup after successful primary write
            let bak_path = PathBuf::from(format!("{}.bak", path.display()));
            let _ = write_atomic(&bak_path, &serialized);
        }
    }
}

/// Applies a successfully-parsed PersistedSettings into state.
fn apply_persisted_settings(state: &mut AppState, parsed: PersistedSettings) {
    if let Some(download_dir) = parsed.download_dir {
        state.settings.download_dir = normalize_download_dir(&download_dir);
    }
    if let Some(max_retries) = parsed.max_retries {
        state.settings.max_retries = max_retries.clamp(0, 10);
    }
    if let Some(language) = parsed.language {
        state.settings.language = language;
    }
    if let Some(max_concurrent) = parsed.max_concurrent_downloads {
        state.settings.max_concurrent_downloads = max_concurrent.clamp(1, 3);
    }
}

/// Loads settings with backup recovery per SPEC-STABILITY-002 REQ-003.
// @MX:NOTE: [AUTO] Replaces load_settings. Adds backup recovery and corruption events per SPEC-STABILITY-002.
fn load_settings_with_recovery(app: &AppHandle, state: &mut AppState) {
    let path = settings_file_path(app);
    let bak_path = PathBuf::from(format!("{}.bak", path.display()));

    match fs::read_to_string(&path) {
        Err(_) => {
            // File missing: use defaults, no events emitted
        }
        Ok(content) => {
            match serde_json::from_str::<PersistedSettings>(&content) {
                Ok(parsed) => {
                    // Valid: apply settings and write backup
                    apply_persisted_settings(state, parsed);
                    if let Ok(serialized) = serde_json::to_string_pretty(&state.settings) {
                        let _ = write_atomic(&bak_path, &serialized);
                    }
                }
                Err(parse_err) => {
                    // Corrupt: try backup
                    match fs::read_to_string(&bak_path) {
                        Ok(bak_content) => {
                            match serde_json::from_str::<PersistedSettings>(&bak_content) {
                                Ok(bak_parsed) => {
                                    // Backup valid: restore from backup
                                    apply_persisted_settings(state, bak_parsed);
                                    // Rewrite primary from restored settings
                                    if let Ok(serialized) =
                                        serde_json::to_string_pretty(&state.settings)
                                    {
                                        let _ = write_atomic(&path, &serialized);
                                    }
                                    let _ = app.emit(
                                        "settings-corruption-recovered",
                                        serde_json::json!({
                                            "message": "Settings restored from backup"
                                        }),
                                    );
                                }
                                Err(_) => {
                                    // Backup also corrupt: use defaults
                                    let _ = app.emit(
                                        "settings-corruption-unrecoverable",
                                        serde_json::json!({
                                            "error": parse_err.to_string()
                                        }),
                                    );
                                }
                            }
                        }
                        Err(_) => {
                            // No backup: use defaults
                            let _ = app.emit(
                                "settings-corruption-unrecoverable",
                                serde_json::json!({
                                    "error": parse_err.to_string()
                                }),
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Normalizes queue items after loading: resets downloading→queued, fills missing logs.
fn normalize_queue_items(items: &mut [QueueItem]) {
    for item in items.iter_mut() {
        if item.status == "downloading" {
            item.status = "queued".to_string();
        }
        if item.download_log.is_none() {
            item.download_log = Some(Vec::new());
        }
    }
}

/// Loads the queue with backup recovery per SPEC-STABILITY-002 REQ-001.
// @MX:NOTE: [AUTO] Replaces load_queue. Adds backup recovery and corruption events per SPEC-STABILITY-002.
fn load_queue_with_recovery(app: &AppHandle, state: &mut AppState) {
    let path = queue_file_path(app);
    let bak_path = PathBuf::from(format!("{}.bak", path.display()));

    match fs::read_to_string(&path) {
        Err(_) => {
            // File missing: empty queue, no events
        }
        Ok(content) => {
            match serde_json::from_str::<Vec<QueueItem>>(&content) {
                Ok(mut parsed) => {
                    // Valid: normalize items, write backup
                    normalize_queue_items(&mut parsed);
                    let backup_count = parsed.len();
                    state.queue = parsed;
                    if let Ok(serialized) = serde_json::to_string_pretty(&state.queue) {
                        let _ = write_atomic(&bak_path, &serialized);
                    }
                    let _ = backup_count; // suppress unused warning if needed
                }
                Err(parse_err) => {
                    // Corrupt: try backup
                    match fs::read_to_string(&bak_path) {
                        Ok(bak_content) => {
                            match serde_json::from_str::<Vec<QueueItem>>(&bak_content) {
                                Ok(mut bak_parsed) => {
                                    // Backup valid: restore
                                    normalize_queue_items(&mut bak_parsed);
                                    let backup_item_count = bak_parsed.len();
                                    state.queue = bak_parsed;
                                    // Rewrite primary from restored queue
                                    if let Ok(serialized) =
                                        serde_json::to_string_pretty(&state.queue)
                                    {
                                        let _ = write_atomic(&path, &serialized);
                                    }
                                    let _ = app.emit(
                                        "queue-corruption-recovered",
                                        serde_json::json!({
                                            "backup_item_count": backup_item_count,
                                            "message": "Queue restored from backup"
                                        }),
                                    );
                                }
                                Err(_) => {
                                    // Backup also corrupt: empty queue
                                    let _ = app.emit(
                                        "queue-corruption-unrecoverable",
                                        serde_json::json!({
                                            "error": parse_err.to_string()
                                        }),
                                    );
                                }
                            }
                        }
                        Err(_) => {
                            // No backup: empty queue
                            let _ = app.emit(
                                "queue-corruption-unrecoverable",
                                serde_json::json!({
                                    "error": parse_err.to_string()
                                }),
                            );
                        }
                    }
                }
            }
        }
    }
}

#[allow(dead_code)]
fn retry_delay_ms(attempt: usize) -> u64 {
    let idx = attempt.min(RETRY_DELAY_TABLE_MS.len().saturating_sub(1));
    RETRY_DELAY_TABLE_MS[idx]
}

// @MX:NOTE: Error classification for smart retry strategy (SPEC-STABILITY-005).
// Pure function: no side effects. Converts raw yt-dlp error strings into a
// RetryStrategy that determines whether and how quickly to retry a download.
#[derive(Debug, PartialEq)]
pub enum RetryStrategy {
    NoRetry,
    RateLimit,
    NetworkError,
    Default,
}

pub fn classify_download_error(error: &str) -> RetryStrategy {
    let lower = error.to_lowercase();
    // Permanent errors — no retry
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
    // Rate-limit errors — long delays
    if lower.contains("http error 429")
        || lower.contains("too many requests")
        || lower.contains("rate limit")
    {
        return RetryStrategy::RateLimit;
    }
    // Transient network errors — short delays
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

pub fn retry_delay_ms_for_strategy(strategy: &RetryStrategy, attempt: usize) -> u64 {
    let table = match strategy {
        RetryStrategy::RateLimit => &RETRY_DELAY_RATE_LIMIT_MS,
        RetryStrategy::NetworkError => &RETRY_DELAY_NETWORK_MS,
        _ => &RETRY_DELAY_TABLE_MS,
    };
    let idx = attempt.min(table.len().saturating_sub(1));
    table[idx]
}

/// Scans the download directory for `.incomplete` marker files at startup.
/// For each marker: finds matching queue item by output_path, marks it failed,
/// then removes the marker regardless of whether a matching item was found.
fn scan_incomplete_markers(app: &AppHandle, state: &mut AppState) {
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

fn expected_extension(mode: &DownloadMode) -> &'static str {
    match mode {
        DownloadMode::Audio => "mp3",
        DownloadMode::Video => "mp4",
    }
}

fn build_unique_output_path(state: &AppState, title: &str, mode: &DownloadMode) -> PathBuf {
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

fn select_format_expression(mode: &DownloadMode, quality_id: &str) -> String {
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

fn append_download_log(item: &mut QueueItem, line: &str) -> bool {
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

fn handle_download_output_line(
    shared: &Arc<Mutex<AppState>>,
    app: &AppHandle,
    job_id: &str,
    line: &str,
) {
    let normalized = line.trim();
    if normalized.is_empty() {
        return;
    }

    let snapshot_to_emit = {
        let mut should_emit = false;
        let mut state = match shared.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::WouldBlock) => return,
            Err(TryLockError::Poisoned(e)) => {
                eprintln!("[STABILITY] Mutex poisoned (try_lock), recovering");
                e.into_inner()
            }
        };

        if let Some(item) = state.queue.iter_mut().find(|queued| queued.id == job_id) {
            let can_update_transfer_state = item.status == "queued" || item.status == "downloading";
            if !can_update_transfer_state {
                return;
            }

            should_emit |= append_download_log(item, normalized);

            if (normalized.contains("ERROR:") || normalized.contains("HTTP Error"))
                && item.error_message.as_deref() != Some(normalized)
            {
                item.error_message = Some(normalized.to_string());
                should_emit = true;
            }
            if let Some(progress) = parse_progress_percent(normalized) {
                if (item.progress_percent - progress).abs() > f64::EPSILON {
                    item.progress_percent = progress;
                    should_emit = true;
                }
                if item.status != "downloading" {
                    item.status = "downloading".to_string();
                    should_emit = true;
                }
            }
            if let Some(speed) = parse_speed(normalized) {
                if item.speed_text.as_deref() != Some(speed.as_str()) {
                    item.speed_text = Some(speed);
                    should_emit = true;
                }
            }
            if let Some(eta) = parse_eta(normalized) {
                if item.eta_text.as_deref() != Some(eta.as_str()) {
                    item.eta_text = Some(eta);
                    should_emit = true;
                }
            }
        }

        if should_emit {
            Some(queue_snapshot(&state))
        } else {
            None
        }
    };

    if let Some(snapshot) = snapshot_to_emit {
        emit_queue_updated_snapshot(app, snapshot);
    }
}

fn kill_active_child_unchecked(runtime: &Arc<Mutex<RuntimeState>>) {
    // Kill all active processes (multi-worker support)
    let children: Vec<Arc<Mutex<Child>>> = runtime
        .lock()
        .ok()
        .map(|mut guard| {
            guard
                .active_processes
                .drain()
                .map(|(_, active)| active.child)
                .collect()
        })
        .unwrap_or_default();
    for child in children {
        if let Ok(mut locked_child) = child.lock() {
            terminate_child_with_grace_period(&mut locked_child);
        }
    }
}

fn try_terminate_child_gracefully(child: &mut Child) -> bool {
    let pid = child.id().to_string();

    #[cfg(unix)]
    {
        let status = Command::new("kill").args(["-INT", &pid]).status();
        status.map(|result| result.success()).unwrap_or(false)
    }

    #[cfg(windows)]
    {
        // Windows has no POSIX SIGINT for arbitrary children without process group setup.
        // Try non-forced taskkill first, then fallback to hard kill below.
        let status = Command::new("taskkill").args(["/PID", &pid, "/T"]).status();
        return status.map(|result| result.success()).unwrap_or(false);
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        false
    }
}

fn terminate_child_with_grace_period(child: &mut Child) {
    let graceful_sent = try_terminate_child_gracefully(child);

    if graceful_sent {
        for _ in 0..10 {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(_) => break,
            }
        }
    }

    // Verify whether the process has already exited before force-killing.
    match child.try_wait() {
        Ok(Some(_)) => {
            // Process already exited; skip force kill.
        }
        Ok(None) => {
            // Process still running; apply force kill.
            eprintln!("[STABILITY] Process still running after grace period; force killing");
            #[cfg(windows)]
            {
                let pid = child.id().to_string();
                let _ = Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid])
                    .status();
            }
            let _ = child.kill();
        }
        Err(_) => {
            // Cannot determine process state; attempt kill as a precaution.
            let _ = child.kill();
        }
    }
}

fn clear_active_process(runtime: &Arc<Mutex<RuntimeState>>, job_id: &str) {
    if let Ok(mut guard) = runtime.lock() {
        guard.active_processes.remove(job_id);
    }
}

fn start_worker_if_needed(
    app: AppHandle,
    shared: Arc<Mutex<AppState>>,
    runtime: Arc<Mutex<RuntimeState>>,
) {
    let should_start = {
        let mut state = lock_or_recover(&shared, "start_worker_if_needed/should_start");
        let max_concurrent = state.settings.max_concurrent_downloads.clamp(1, 3) as usize;
        if state.active_worker_count >= max_concurrent {
            false
        } else if state.queue.iter().any(|item| item.status == "queued") {
            state.active_worker_count += 1;
            true
        } else {
            false
        }
    };

    if !should_start {
        return;
    }

    // Create a shutdown channel for graceful worker termination.
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel::<()>();
    {
        let mut rt = lock_or_recover(&runtime, "start_worker_if_needed/shutdown_tx");
        rt.shutdown_txs.push(shutdown_tx);
    }

    // Clone runtime Arc so the closure can own one reference and we keep another for handle storage.
    let runtime_for_thread = runtime.clone();
    let handle = std::thread::spawn(move || {
        let runtime = runtime_for_thread;
        loop {
            // Check for shutdown signal before processing the next job.
            if shutdown_rx.try_recv().is_ok() {
                eprintln!("[STABILITY] Worker thread received shutdown signal; exiting");
                let mut state = lock_or_recover(&shared, "worker_thread/shutdown");
                state.active_worker_count = state.active_worker_count.saturating_sub(1);
                return;
            }
            let current_job = {
                let mut state = lock_or_recover(&shared, "worker_thread/current_job");
                let next_index = state.queue.iter().position(|item| item.status == "queued");
                if let Some(index) = next_index {
                    state.queue[index].status = "downloading".to_string();
                    state.queue[index].progress_percent = 0.0;
                    let job = state.queue[index].clone();
                    emit_queue_updated(&app, &state);
                    Some(job)
                } else {
                    state.active_worker_count = state.active_worker_count.saturating_sub(1);
                    emit_queue_updated(&app, &state);
                    None
                }
            };

            let Some(job) = current_job else {
                return;
            };

            if let Some(dependency) = app.try_state::<SharedDependencyState>() {
                if let Err(err) = wait_for_dependencies(&app, &dependency.0) {
                    let mut state = lock_or_recover(&shared, "worker_thread/wait_for_deps_failure");
                    if let Some(item) = state.queue.iter_mut().find(|item| item.id == job.id) {
                        item.status = "failed".to_string();
                        item.error_message = Some(err);
                    }
                    emit_queue_updated(&app, &state);
                    persist_queue(&app, &state);
                    continue;
                }
            }

            let (download_dir, final_output_path, max_retries) = {
                let state = lock_or_recover(&shared, "worker_thread/download_setup");
                let path = build_unique_output_path(&state, &job.title, &job.mode);
                (
                    state.settings.download_dir.clone(),
                    path,
                    state.settings.max_retries.max(0) as usize,
                )
            };

            let _ = fs::create_dir_all(&download_dir);
            let temp_dir = temp_job_dir_path(&app, &job.id);
            let _ = fs::create_dir_all(&temp_dir);
            let output_template = temp_dir.join("media.%(ext)s");

            let format_expr = select_format_expression(&job.mode, &job.quality_id);
            let mut args = vec![
                "--no-playlist".to_string(),
                "--newline".to_string(),
                "--progress".to_string(),
                "--socket-timeout".to_string(),
                "30".to_string(),
                "--fragment-retries".to_string(),
                "10".to_string(),
                "--throttled-rate".to_string(),
                "100K".to_string(),
                "--extractor-retries".to_string(),
                "5".to_string(),
                "--concurrent-fragments".to_string(),
                "4".to_string(),
                "-f".to_string(),
                format_expr,
                "-o".to_string(),
                output_template.to_string_lossy().to_string(),
                job.url.clone(),
            ];

            match job.mode {
                DownloadMode::Audio => {
                    args.push("-x".to_string());
                    args.push("--audio-format".to_string());
                    args.push("mp3".to_string());
                }
                DownloadMode::Video => {
                    args.push("--merge-output-format".to_string());
                    args.push("mp4".to_string());
                    args.push("--recode-video".to_string());
                    args.push("mp4".to_string());
                }
            }

            let mut attempt: usize = 0;
            loop {
                {
                    let state = lock_or_recover(&shared, "worker_thread/retry_loop_stop_check");
                    let stopped = state
                        .queue
                        .iter()
                        .find(|item| item.id == job.id)
                        .map(|item| item.status == "paused" || item.status == "canceled")
                        .unwrap_or(true);
                    if stopped {
                        break;
                    }
                }

                let yt_dlp = resolve_executable(&app, "yt-dlp");
                let mut cmd = Command::new(&yt_dlp);
                configure_hidden_process(&mut cmd);
                let spawn_result = cmd
                    .args(args.clone())
                    .env("PATH", managed_path_env(&app))
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn();

                let (process_ok, process_error): (bool, Option<String>) = match spawn_result {
                    Ok(mut child) => {
                        let stdout_reader = child.stdout.take().map(BufReader::new);
                        let stderr_reader = child.stderr.take().map(BufReader::new);

                        let child_arc = Arc::new(Mutex::new(child));
                        {
                            let mut guard =
                                lock_or_recover(&runtime, "worker_thread/active_processes_insert");
                            guard.active_processes.insert(
                                job.id.clone(),
                                ActiveProcess {
                                    job_id: job.id.clone(),
                                    child: child_arc.clone(),
                                },
                            );
                        }

                        let shared_stdout = shared.clone();
                        let app_stdout = app.clone();
                        let job_id_stdout = job.id.clone();
                        let stdout_thread = stdout_reader.map(|reader| {
                            std::thread::spawn(move || {
                                for line in reader.lines().map_while(Result::ok) {
                                    handle_download_output_line(
                                        &shared_stdout,
                                        &app_stdout,
                                        &job_id_stdout,
                                        &line,
                                    );
                                }
                            })
                        });

                        let shared_stderr = shared.clone();
                        let app_stderr = app.clone();
                        let job_id_stderr = job.id.clone();
                        let stderr_thread = stderr_reader.map(|reader| {
                            std::thread::spawn(move || {
                                for line in reader.lines().map_while(Result::ok) {
                                    handle_download_output_line(
                                        &shared_stderr,
                                        &app_stderr,
                                        &job_id_stderr,
                                        &line,
                                    );
                                }
                            })
                        });

                        let wait_result = loop {
                            let status = {
                                // Keep the child lock only for try_wait, so pause/cancel can acquire it.
                                let mut locked_child =
                                    lock_or_recover(&child_arc, "worker_thread/child_try_wait");
                                locked_child.try_wait()
                            };

                            match status {
                                Ok(Some(exit_status)) => break Ok(exit_status),
                                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                                Err(err) => break Err(err),
                            }
                        };

                        if let Some(handle) = stdout_thread {
                            let _ = handle.join();
                        }
                        if let Some(handle) = stderr_thread {
                            let _ = handle.join();
                        }

                        clear_active_process(&runtime, &job.id);

                        match wait_result {
                            Ok(exit_status) => (exit_status.success(), None),
                            Err(err) => (false, Some(err.to_string())),
                        }
                    }
                    Err(err) => (false, Some(err.to_string())),
                };

                let mut should_retry = false;
                let mut should_retry_strategy = RetryStrategy::Default;
                {
                    let mut state = lock_or_recover(&shared, "worker_thread/retry_result");
                    if let Some(item) = state.queue.iter_mut().find(|item| item.id == job.id) {
                        if item.status == "paused" || item.status == "canceled" {
                            // Keep paused/canceled state as-is.
                        } else if process_ok {
                            let expected_ext = expected_extension(&job.mode);
                            let move_result = resolve_downloaded_file_path(&temp_dir, expected_ext)
                                .and_then(|completed_path| {
                                    move_file_atomic(&completed_path, &final_output_path)
                                });
                            match move_result {
                                Ok(()) => {
                                    item.status = "completed".to_string();
                                    item.progress_percent = 100.0;
                                    item.output_path =
                                        Some(final_output_path.to_string_lossy().to_string());
                                    item.error_message = None;
                                }
                                Err(err) => {
                                    item.status = "failed".to_string();
                                    item.error_message = Some(err);
                                }
                            }
                        } else {
                            let fallback =
                                process_error.unwrap_or_else(|| "다운로드 실패".to_string());
                            item.error_message = Some(fallback.clone());
                            let strategy = classify_download_error(&fallback);
                            if strategy == RetryStrategy::NoRetry {
                                item.status = "failed".to_string();
                            } else if attempt < max_retries {
                                should_retry = true;
                                should_retry_strategy = strategy;
                                item.retry_count = (attempt + 1) as i32;
                                item.status = "queued".to_string();
                                item.speed_text = None;
                                item.eta_text = None;
                            } else {
                                item.status = "failed".to_string();
                            }
                        }
                    }
                    emit_queue_updated(&app, &state);
                    persist_queue(&app, &state);
                }

                if should_retry {
                    attempt += 1;
                    std::thread::sleep(Duration::from_millis(retry_delay_ms_for_strategy(
                        &should_retry_strategy,
                        attempt,
                    )));
                    continue;
                }
                break;
            }

            remove_directory_safe(&temp_dir);
        } // end loop
    }); // end thread closure

    // Store the worker handle for graceful join on shutdown.
    {
        let mut rt = lock_or_recover(&runtime, "start_worker_if_needed/worker_handle");
        rt.worker_handles.push(handle);
    }
}

#[tauri::command]
async fn analyze_url(
    app: AppHandle,
    dependency: State<'_, SharedDependencyState>,
    url: String,
) -> CommandResult<AnalysisResult> {
    let normalized_url = normalize_youtube_video_url(&url);
    if normalized_url.trim().is_empty() {
        return Err("URL is empty".to_string());
    }

    wait_for_dependencies(&app, &dependency.0)?;

    let yt_dlp = resolve_executable(&app, "yt-dlp");
    let output = run_command_capture(
        &app,
        &yt_dlp,
        &[
            "--no-playlist",
            "-J",
            "--no-warnings",
            normalized_url.trim(),
        ],
        ANALYZE_TIMEOUT_MS,
    );

    if output.code != 0 {
        let stderr = output.stderr.trim().to_string();
        return Err(if stderr.is_empty() {
            "URL 분석에 실패했습니다.".to_string()
        } else {
            stderr
        });
    }

    let payload: Value = serde_json::from_str(&output.stdout).map_err(|err| err.to_string())?;
    let is_live = payload
        .get("is_live")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let live_status = payload
        .get("live_status")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    if is_live || live_status == "is_live" {
        return Err("현재 라이브 스트리밍 중인 영상은 다운로드할 수 없습니다.".to_string());
    }

    let formats = payload
        .get("formats")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut video_candidates: Vec<(i64, i64, QualityOption)> = Vec::new();
    let mut audio_candidates: Vec<(i64, QualityOption)> = Vec::new();

    for format in formats {
        let format_id = format
            .get("format_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let ext = format
            .get("ext")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let height = format.get("height").and_then(Value::as_i64).unwrap_or(0);
        let fps = format.get("fps").and_then(Value::as_i64).unwrap_or(0);
        let tbr = format.get("tbr").and_then(Value::as_f64).unwrap_or(0.0);
        let abr = format.get("abr").and_then(Value::as_f64).unwrap_or(0.0);
        let vcodec = format
            .get("vcodec")
            .and_then(Value::as_str)
            .unwrap_or("none")
            .to_string();
        let acodec = format
            .get("acodec")
            .and_then(Value::as_str)
            .unwrap_or("none")
            .to_string();

        if vcodec != "none" && height > 0 {
            let ext_priority = if ext == "mp4" {
                2
            } else if ext == "webm" {
                1
            } else {
                0
            };
            let quality_rank = height * 1_000_000 + fps * 1_000 + tbr as i64;
            video_candidates.push((
                ext_priority,
                quality_rank,
                QualityOption {
                    id: format_id.clone(),
                    label: format!("{height}p"),
                    ext: ext.clone(),
                    mode: DownloadMode::Video,
                },
            ));
        }
        if acodec != "none" && vcodec == "none" {
            let abr_value = abr.floor() as i64;
            let quality_rank = abr_value * 1000 + tbr as i64;
            audio_candidates.push((
                quality_rank,
                QualityOption {
                    id: format_id,
                    label: format!("{abr_value}kbps"),
                    ext,
                    mode: DownloadMode::Audio,
                },
            ));
        }
    }

    video_candidates.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    audio_candidates.sort_by(|a, b| b.0.cmp(&a.0));

    let mut seen_heights = HashSet::new();
    let mut video_options: Vec<QualityOption> = Vec::new();
    for (_, _, option) in video_candidates {
        let height = option
            .label
            .strip_suffix('p')
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
        if seen_heights.contains(&height) {
            continue;
        }
        seen_heights.insert(height);
        video_options.push(option);
    }
    if video_options.is_empty() {
        video_options.push(QualityOption {
            id: "bestvideo+bestaudio".to_string(),
            label: "Best Video".to_string(),
            ext: "mp4".to_string(),
            mode: DownloadMode::Video,
        });
    }

    let mut audio_options: Vec<QualityOption> = audio_candidates
        .into_iter()
        .map(|(_, option)| option)
        .collect();
    if audio_options.is_empty() {
        audio_options.push(QualityOption {
            id: "bestaudio".to_string(),
            label: "Best Audio".to_string(),
            ext: "m4a".to_string(),
            mode: DownloadMode::Audio,
        });
    }

    Ok(AnalysisResult {
        source_url: normalized_url,
        title: payload
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Unknown Title")
            .to_string(),
        channel: payload
            .get("uploader")
            .and_then(Value::as_str)
            .unwrap_or("Unknown Channel")
            .to_string(),
        duration_sec: payload.get("duration").and_then(Value::as_i64).unwrap_or(0),
        thumbnail_url: payload
            .get("thumbnail")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        video_options,
        audio_options,
    })
}

#[tauri::command]
async fn check_duplicate(
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

#[tauri::command]
async fn enqueue_job(
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

#[tauri::command]
async fn pause_job(
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

    // Prioritize UI state update, then stop the active process asynchronously.
    let runtime = runtime.0.clone();
    std::thread::spawn(move || {
        kill_active_child_unchecked(&runtime);
    });

    Ok(snapshot)
}

#[tauri::command]
async fn resume_job(
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

#[tauri::command]
async fn cancel_job(
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

    // Keep cancel behavior aligned with pause: stop active process aggressively in background.
    let runtime = runtime.0.clone();
    std::thread::spawn(move || {
        kill_active_child_unchecked(&runtime);
    });

    Ok(snapshot)
}

#[tauri::command]
async fn clear_terminal_jobs(
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

#[tauri::command]
async fn get_queue_snapshot(state: State<'_, SharedState>) -> CommandResult<QueueSnapshot> {
    let state = state
        .0
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    Ok(queue_snapshot(&state))
}

#[tauri::command]
async fn get_settings(state: State<'_, SharedState>) -> CommandResult<AppSettings> {
    let state = state
        .0
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    Ok(state.settings.clone())
}

#[tauri::command]
async fn get_dependency_bootstrap_status(
    dependency: State<'_, SharedDependencyState>,
) -> CommandResult<DependencyBootstrapStatus> {
    let state = dependency
        .0
        .lock()
        .map_err(|_| "dependency state lock poisoned".to_string())?;
    Ok(state.status.clone())
}

#[tauri::command]
async fn pick_download_dir() -> CommandResult<Option<String>> {
    let selected = FileDialog::new().pick_folder();
    Ok(selected.map(|path| path.to_string_lossy().to_string()))
}

#[tauri::command]
async fn set_settings(
    app: AppHandle,
    state: State<'_, SharedState>,
    settings: AppSettings,
) -> CommandResult<()> {
    let mut state = state
        .0
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    state.settings = AppSettings {
        download_dir: normalize_download_dir(&settings.download_dir),
        max_retries: settings.max_retries.clamp(0, 10),
        language: settings.language,
        max_concurrent_downloads: settings.max_concurrent_downloads.clamp(1, 3),
    };
    persist_settings(&app, &state.settings);
    Ok(())
}

#[tauri::command]
async fn delete_file(
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

pub fn run() {
    let builder = tauri::Builder::default()
        .setup(|app| {
            remove_directory_safe(&temp_downloads_root_dir(app.app_handle()));

            let mut initial_state = AppState {
                queue: Vec::new(),
                settings: default_settings(),
                active_worker_count: 0,
            };
            load_settings_with_recovery(app.app_handle(), &mut initial_state);
            load_queue_with_recovery(app.app_handle(), &mut initial_state);
            scan_incomplete_markers(app.app_handle(), &mut initial_state);
            app.manage(SharedState(Arc::new(Mutex::new(initial_state))));
            app.manage(SharedRuntime(Arc::new(Mutex::new(RuntimeState::default()))));
            let dependency_state = Arc::new(Mutex::new(DependencyRuntimeState {
                status: default_dependency_status(),
                dependencies_ready: false,
                in_progress: false,
            }));
            app.manage(SharedDependencyState(dependency_state.clone()));

            if let Some(state) = app.try_state::<SharedState>() {
                if let Ok(locked) = state.0.lock() {
                    emit_queue_updated(app.app_handle(), &locked);
                }
            }
            emit_dependency_status(
                app.app_handle(),
                &DependencyBootstrapStatus {
                    in_progress: true,
                    phase: "preparing".to_string(),
                    progress_percent: Some(5),
                    error_message: None,
                },
            );
            start_dependency_bootstrap_if_needed(app.app_handle().clone(), dependency_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            analyze_url,
            check_duplicate,
            enqueue_job,
            pause_job,
            resume_job,
            cancel_job,
            clear_terminal_jobs,
            get_queue_snapshot,
            get_settings,
            get_dependency_bootstrap_status,
            pick_download_dir,
            set_settings,
            run_diagnostics,
            check_update,
            get_storage_stats,
            delete_file,
            open_folder,
            open_external_url
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                // Send shutdown signal to all worker threads when the window is destroyed.
                let app = window.app_handle();
                if let Some(runtime) = app.try_state::<SharedRuntime>() {
                    if let Ok(mut rt) = runtime.0.lock() {
                        let txs: Vec<_> = rt.shutdown_txs.drain(..).collect();
                        for tx in txs {
                            let _ = tx.send(());
                        }
                        eprintln!(
                            "[STABILITY] Sent shutdown signal to {} worker thread(s)",
                            rt.worker_handles.len()
                        );
                    }
                }
            }
        });

    if let Err(e) = builder.run(tauri::generate_context!()) {
        eprintln!("[FATAL] Tauri initialization failed: {:?}", e);
        std::process::exit(1);
    }
}
