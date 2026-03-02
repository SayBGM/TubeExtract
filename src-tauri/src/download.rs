// @MX:ANCHOR: Central download worker module — all yt-dlp process management lives here.
// @MX:REASON: start_worker_if_needed, kill_active_child_unchecked, and RuntimeState are
//             referenced by queue.rs commands and lib.rs run(); high fan_in boundary.

use crate::file_ops::{
    configure_hidden_process, managed_path_env, move_file_atomic, remove_directory_safe,
    resolve_downloaded_file_path, resolve_executable, temp_job_dir_path,
};
use crate::queue::{
    append_download_log, build_unique_output_path, emit_queue_updated, emit_queue_updated_snapshot,
    expected_extension, persist_queue, queue_snapshot, select_format_expression,
};
use crate::state::lock_or_recover;
use crate::utils::{parse_eta, parse_progress_percent, parse_speed};
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, TryLockError};
use std::time::Duration;
use tauri::{AppHandle, Manager};

// Delay tables (ms) indexed by retry attempt (clamped to table length).
const RETRY_DELAY_TABLE_MS: [u64; 4] = [2000, 5000, 10000, 15000];
const RETRY_DELAY_RATE_LIMIT_MS: [u64; 4] = [30_000, 60_000, 120_000, 120_000];
const RETRY_DELAY_NETWORK_MS: [u64; 4] = [1_000, 2_000, 5_000, 10_000];

// ============================================================================
// Runtime state types
// ============================================================================

/// Holds a reference to a running yt-dlp subprocess for a specific job.
#[derive(Clone)]
pub struct ActiveProcess {
    #[allow(dead_code)]
    pub job_id: String,
    pub child: Arc<Mutex<Child>>,
}

/// Mutable runtime state tracking active subprocesses and worker thread handles.
#[derive(Default)]
pub struct RuntimeState {
    pub active_processes: std::collections::HashMap<String, ActiveProcess>,
    // Shutdown senders for graceful worker thread termination.
    pub shutdown_txs: Vec<std::sync::mpsc::Sender<()>>,
    // Handles for worker threads.
    pub worker_handles: Vec<std::thread::JoinHandle<()>>,
}

/// Thread-safe shared handle for the download worker runtime state.
#[derive(Clone)]
pub struct SharedRuntime(pub Arc<Mutex<RuntimeState>>);

// ============================================================================
// Smart retry strategy (SPEC-STABILITY-005)
// ============================================================================

// @MX:NOTE: Error classification for smart retry strategy (SPEC-STABILITY-005).
// Pure function: no side effects. Converts raw yt-dlp error strings into a
// RetryStrategy that determines whether and how quickly to retry a download.
/// Retry behaviour derived from yt-dlp error output classification.
#[derive(Debug, PartialEq)]
pub enum RetryStrategy {
    NoRetry,
    RateLimit,
    NetworkError,
    Default,
}

/// Classifies a yt-dlp error string into a RetryStrategy.
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

/// Returns the delay in milliseconds before retrying based on strategy and attempt number.
pub fn retry_delay_ms_for_strategy(strategy: &RetryStrategy, attempt: usize) -> u64 {
    let table = match strategy {
        RetryStrategy::RateLimit => &RETRY_DELAY_RATE_LIMIT_MS,
        RetryStrategy::NetworkError => &RETRY_DELAY_NETWORK_MS,
        _ => &RETRY_DELAY_TABLE_MS,
    };
    let idx = attempt.min(table.len().saturating_sub(1));
    table[idx]
}

// ============================================================================
// Worker process management
// ============================================================================

/// Kills all active yt-dlp subprocesses without waiting for graceful shutdown.
pub fn kill_active_child_unchecked(runtime: &Arc<Mutex<RuntimeState>>) {
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

    match child.try_wait() {
        Ok(Some(_)) => {
            // Process already exited; skip force kill.
        }
        Ok(None) => {
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
            let _ = child.kill();
        }
    }
}

fn clear_active_process(runtime: &Arc<Mutex<RuntimeState>>, job_id: &str) {
    if let Ok(mut guard) = runtime.lock() {
        guard.active_processes.remove(job_id);
    }
}

// ============================================================================
// Download output line handler
// ============================================================================

fn handle_download_output_line(
    shared: &Arc<Mutex<crate::settings::AppState>>,
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

// ============================================================================
// Worker thread
// ============================================================================

// @MX:ANCHOR: Main download worker entry point — called by queue commands (enqueue, resume)
// and by lib.rs setup for queue recovery. All yt-dlp spawning flows through here.
// @MX:REASON: Called from queue::enqueue_job, queue::resume_job, and lib.rs setup; fan_in >= 3.
pub fn start_worker_if_needed(
    app: AppHandle,
    shared: Arc<Mutex<crate::settings::AppState>>,
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

    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel::<()>();
    {
        let mut rt = lock_or_recover(&runtime, "start_worker_if_needed/shutdown_tx");
        rt.shutdown_txs.push(shutdown_tx);
    }

    let runtime_for_thread = runtime.clone();
    let handle = std::thread::spawn(move || {
        let runtime = runtime_for_thread;
        loop {
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

            if let Some(dependency) = app.try_state::<crate::dependencies::SharedDependencyState>()
            {
                if let Err(err) = crate::dependencies::wait_for_dependencies(&app, &dependency.0) {
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
                crate::metadata::DownloadMode::Audio => {
                    args.push("-x".to_string());
                    args.push("--audio-format".to_string());
                    args.push("mp3".to_string());
                }
                crate::metadata::DownloadMode::Video => {
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
        }
    });

    {
        let mut rt = lock_or_recover(&runtime, "start_worker_if_needed/worker_handle");
        rt.worker_handles.push(handle);
    }
}

// Re-export resolve_downloaded_file_path for use in file_ops resolution within the worker.
// (Already available via crate::file_ops — no re-export needed.)
