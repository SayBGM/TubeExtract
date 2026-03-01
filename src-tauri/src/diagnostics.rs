use fs2::statvfs;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, State};
use url::Url;

use crate::dependencies::{wait_for_dependencies, SharedDependencyState};
use crate::file_ops::resolve_executable;
use crate::file_ops::run_command_capture;
use crate::types::CommandResult;

// ============================================================================
// Constants
// ============================================================================

const DIAGNOSTICS_COMMAND_TIMEOUT_MS: u64 = 10_000;

// ============================================================================
// Result types
// ============================================================================

/// Result of the `run_diagnostics` command.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsResult {
    pub yt_dlp_available: bool,
    pub ffmpeg_available: bool,
    pub download_path_writable: bool,
    pub message: String,
}

/// Disk storage statistics for the download directory.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub used_percent: f64,
    pub download_dir_bytes: u64,
}

// ============================================================================
// Helper functions
// ============================================================================

/// Returns `true` if the given directory is writable by attempting to create a test file.
pub fn can_write_to_dir(dir: &Path) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis();
    let test_file = dir.join(format!("tubeextract_write_test_{now}.tmp"));
    let write_result = fs::write(&test_file, "ok");
    if write_result.is_err() {
        return false;
    }
    let _ = fs::remove_file(test_file);
    true
}

/// Truncates a reason string to at most 120 characters.
pub fn truncate_reason(reason: &str) -> String {
    reason.chars().take(120).collect()
}

/// Recursively calculates the total byte size of all files in a directory.
pub fn calculate_directory_size(path: &Path) -> u64 {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };

    let mut total: u64 = 0;
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_file() {
            total = total.saturating_add(metadata.len());
        } else if metadata.is_dir() {
            total = total.saturating_add(calculate_directory_size(&entry.path()));
        }
    }
    total
}

/// Opens a path or URL using the platform's native opener (open/explorer/xdg-open).
fn open_with_platform_command(
    target: &str,
    is_file_reveal: bool,
    original_path: Option<&Path>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if is_file_reveal {
            if let Some(path) = original_path {
                if path.exists() && path.is_file() {
                    Command::new("open")
                        .arg("-R")
                        .arg(path)
                        .spawn()
                        .map_err(|err| err.to_string())?;
                    return Ok(());
                }
            }
        }
        Command::new("open")
            .arg(target)
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        if is_file_reveal {
            if let Some(path) = original_path {
                if path.exists() && path.is_file() {
                    let selected = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
                    Command::new("explorer")
                        .arg("/select,")
                        .arg(selected)
                        .spawn()
                        .map_err(|err| err.to_string())?;
                    return Ok(());
                }
            }
        }
        Command::new("explorer")
            .arg(target)
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = is_file_reveal;
        let _ = original_path;
        Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

// ============================================================================
// Tauri commands
// ============================================================================

/// Runs system diagnostics: checks yt-dlp, ffmpeg, and download directory writability.
#[tauri::command]
pub async fn run_diagnostics(
    app: AppHandle,
    state: State<'_, crate::SharedState>,
    dependency: State<'_, SharedDependencyState>,
) -> CommandResult<DiagnosticsResult> {
    let dependency_error_message = wait_for_dependencies(&app, &dependency.0).err();
    let state = state
        .0
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    let yt_dlp = resolve_executable(&app, "yt-dlp");
    let ffmpeg = resolve_executable(&app, "ffmpeg");
    let yt = run_command_capture(
        &app,
        &yt_dlp,
        &["--version"],
        DIAGNOSTICS_COMMAND_TIMEOUT_MS,
    );
    let ff = run_command_capture(&app, &ffmpeg, &["-version"], DIAGNOSTICS_COMMAND_TIMEOUT_MS);
    let yt_ok = yt.code == 0;
    let ff_ok = ff.code == 0;

    let download_dir = PathBuf::from(&state.settings.download_dir);
    let _ = fs::create_dir_all(&download_dir);
    let writable = can_write_to_dir(&download_dir);

    let yt_reason = if yt_ok {
        String::new()
    } else if yt.timed_out {
        "timeout".to_string()
    } else {
        yt.stderr.trim().to_string()
    };
    let ff_reason = if ff_ok {
        String::new()
    } else if ff.timed_out {
        "timeout".to_string()
    } else {
        ff.stderr.trim().to_string()
    };

    Ok(DiagnosticsResult {
        yt_dlp_available: yt_ok,
        ffmpeg_available: ff_ok,
        download_path_writable: writable,
        message: {
            let yt_status = if yt_ok { "OK" } else { "FAIL" };
            let yt_detail = if yt_ok {
                String::new()
            } else {
                format!(" ({})", truncate_reason(&yt_reason))
            };
            let ff_status = if ff_ok { "OK" } else { "FAIL" };
            let ff_detail = if ff_ok {
                String::new()
            } else {
                format!(" ({})", truncate_reason(&ff_reason))
            };
            let writable_status = if writable { "OK" } else { "FAIL" };
            let bootstrap_detail = dependency_error_message
                .map(|message| format!(", bootstrap: {message}"))
                .unwrap_or_default();
            format!(
                "yt-dlp: {yt_status}{yt_detail} ({yt_dlp}), ffmpeg: {ff_status}{ff_detail} ({ffmpeg}), download-dir writable: {writable_status}{bootstrap_detail}"
            )
        },
    })
}

/// Returns a stub update check result (no updates available).
#[tauri::command]
pub async fn check_update() -> CommandResult<Value> {
    Ok(serde_json::json!({
      "hasUpdate": false,
      "latestVersion": Value::Null,
      "url": Value::Null
    }))
}

/// Returns disk storage statistics for the download directory.
#[tauri::command]
pub async fn get_storage_stats(
    state: State<'_, crate::SharedState>,
) -> CommandResult<StorageStats> {
    let state = state
        .0
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    let download_dir = PathBuf::from(&state.settings.download_dir);
    let _ = fs::create_dir_all(&download_dir);

    let stat = statvfs(&download_dir).map_err(|err| err.to_string())?;
    let total_bytes = stat.total_space();
    let available_bytes = stat.available_space();
    let used_bytes = total_bytes.saturating_sub(available_bytes);
    let used_percent = if total_bytes == 0 {
        0.0
    } else {
        (used_bytes as f64 / total_bytes as f64) * 100.0
    };
    let download_dir_bytes = calculate_directory_size(&download_dir);

    Ok(StorageStats {
        total_bytes,
        available_bytes,
        used_bytes,
        used_percent,
        download_dir_bytes,
    })
}

/// Opens a folder in the system file manager, or reveals the parent folder of a file.
#[tauri::command]
pub async fn open_folder(path: String) -> CommandResult<()> {
    if path.trim().is_empty() {
        return Ok(());
    }
    let normalized = PathBuf::from(path.trim());
    let target = if normalized.exists() {
        fs::canonicalize(&normalized).unwrap_or_else(|_| normalized.clone())
    } else {
        let parent = normalized
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(path.trim()));
        if parent.exists() {
            fs::canonicalize(&parent).unwrap_or(parent)
        } else {
            parent
        }
    };

    open_with_platform_command(&target.to_string_lossy(), true, Some(&normalized))
}

/// Opens an http/https URL in the system's default browser.
#[tauri::command]
pub async fn open_external_url(url: String) -> CommandResult<()> {
    if url.trim().is_empty() {
        return Ok(());
    }
    let parsed = Url::parse(url.trim()).map_err(|_| "유효한 URL이 아닙니다.".to_string())?;
    let scheme = parsed.scheme().to_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err("http/https URL만 열 수 있습니다.".to_string());
    }

    let target = parsed.to_string();
    open_with_platform_command(&target, false, None)
}
