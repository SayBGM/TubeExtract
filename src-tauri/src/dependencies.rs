use reqwest::blocking::Client;
use serde::Serialize;
use std::fs;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::file_ops::{
    managed_bin_dir_path, managed_executable_path, resolve_executable, run_command_capture,
};

// ============================================================================
// Constants
// ============================================================================

const YTDLP_LATEST_API_URL: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
const YTDLP_VERSION_CHECK_TIMEOUT_MS: u64 = 5_000;
const DIAGNOSTICS_COMMAND_TIMEOUT_MS: u64 = 10_000;
const YTDLP_DOWNLOAD_URL_WINDOWS: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const YTDLP_DOWNLOAD_URL_MACOS: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos";
const YTDLP_DOWNLOAD_URL_LINUX: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux";

#[cfg(target_os = "windows")]
const FFMPEG_DOWNLOAD_URL_WINDOWS: &str =
    "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip";

// ============================================================================
// Types
// ============================================================================

/// Tracks the current state of the dependency bootstrap process.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyBootstrapStatus {
    pub in_progress: bool,
    pub phase: String,
    pub progress_percent: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Runtime state for the dependency bootstrap thread.
#[derive(Debug, Clone)]
pub struct DependencyRuntimeState {
    pub status: DependencyBootstrapStatus,
    pub dependencies_ready: bool,
    pub in_progress: bool,
}

// ============================================================================
// Status helpers
// ============================================================================

/// Returns the default (idle) dependency status.
pub fn default_dependency_status() -> DependencyBootstrapStatus {
    DependencyBootstrapStatus {
        in_progress: false,
        phase: "idle".to_string(),
        progress_percent: None,
        error_message: None,
    }
}

/// Emits a `dependency-bootstrap-updated` event to the frontend.
pub fn emit_dependency_status(app: &AppHandle, status: &DependencyBootstrapStatus) {
    let _ = app.emit("dependency-bootstrap-updated", status.clone());
}

/// Updates the shared dependency state and emits the new status to the frontend.
pub fn set_dependency_status(
    app: &AppHandle,
    dependency: &Arc<Mutex<DependencyRuntimeState>>,
    in_progress: bool,
    phase: &str,
    progress_percent: Option<i32>,
    error_message: Option<String>,
) {
    let next = DependencyBootstrapStatus {
        in_progress,
        phase: phase.to_string(),
        progress_percent,
        error_message,
    };

    if let Ok(mut state) = dependency.lock() {
        state.status = next.clone();
    }
    emit_dependency_status(app, &next);
}

// ============================================================================
// Version helpers
// ============================================================================

/// Strips leading 'v' and surrounding whitespace from a version string.
pub fn normalize_ytdlp_version(input: &str) -> String {
    input.trim().trim_start_matches('v').to_string()
}

/// Returns the installed yt-dlp version at `target`, or `None` if it cannot be run.
pub fn installed_ytdlp_version(app: &AppHandle, target: &std::path::Path) -> Option<String> {
    let target_str = target.to_string_lossy().to_string();
    let check = run_command_capture(
        app,
        &target_str,
        &["--version"],
        YTDLP_VERSION_CHECK_TIMEOUT_MS,
    );
    if check.code != 0 || check.timed_out {
        return None;
    }
    let version = normalize_ytdlp_version(&check.stdout);
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

/// Queries the GitHub releases API for the latest yt-dlp version tag.
pub fn latest_ytdlp_version() -> Option<String> {
    let client = Client::builder()
        .timeout(Duration::from_millis(YTDLP_VERSION_CHECK_TIMEOUT_MS))
        .build()
        .ok()?;
    let payload = client
        .get(YTDLP_LATEST_API_URL)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "TubeExtract")
        .send()
        .ok()?
        .json::<serde_json::Value>()
        .ok()?;
    let tag = payload
        .get("tag_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let version = normalize_ytdlp_version(tag);
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

// ============================================================================
// Download helper
// ============================================================================

/// Downloads a file from `url` to `destination`, calling `on_progress` with
/// `(downloaded_bytes, total_bytes_option)` after each chunk.
pub fn download_file(
    client: &Client,
    url: &str,
    destination: &std::path::Path,
    on_progress: impl Fn(u64, Option<u64>),
) -> Result<(), String> {
    let mut response = client
        .get(url)
        .header("User-Agent", "TubeExtract")
        .send()
        .map_err(|err| err.to_string())?;
    if !response.status().is_success() {
        return Err(format!("파일 다운로드 실패: {}", response.status()));
    }

    let total = response.content_length();
    let mut file = fs::File::create(destination).map_err(|err| err.to_string())?;
    let mut downloaded: u64 = 0;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = response.read(&mut buffer).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|err| err.to_string())?;
        downloaded = downloaded.saturating_add(read as u64);
        on_progress(downloaded, total);
    }
    Ok(())
}

// ============================================================================
// Bootstrap logic
// ============================================================================

/// Ensures yt-dlp is available, downloading it if necessary.
///
/// If `check_latest` is true and yt-dlp is already managed, checks for a newer
/// version and re-downloads if a newer version is available.
pub fn ensure_ytdlp(
    app: &AppHandle,
    dependency: &Arc<Mutex<DependencyRuntimeState>>,
    check_latest: bool,
) -> Result<(), String> {
    set_dependency_status(app, dependency, true, "checking_yt_dlp", Some(15), None);

    let resolved = resolve_executable(app, "yt-dlp");
    let check_existing = run_command_capture(
        app,
        &resolved,
        &["--version"],
        YTDLP_VERSION_CHECK_TIMEOUT_MS,
    );
    if check_existing.code == 0 {
        // If yt-dlp exists (bundled/system/managed), use it as-is.
        return Ok(());
    }

    let target = managed_executable_path(app, "yt-dlp");
    let _ = fs::create_dir_all(managed_bin_dir_path(app));

    if target.exists() {
        if !check_latest {
            return Ok(());
        }
        let installed = installed_ytdlp_version(app, &target);
        if let Some(installed_version) = installed {
            if let Some(latest_version) = latest_ytdlp_version() {
                if installed_version == latest_version {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        }
        let _ = fs::remove_file(&target);
    }

    let download_url = if cfg!(target_os = "windows") {
        YTDLP_DOWNLOAD_URL_WINDOWS
    } else if cfg!(target_os = "macos") {
        YTDLP_DOWNLOAD_URL_MACOS
    } else {
        YTDLP_DOWNLOAD_URL_LINUX
    };

    set_dependency_status(app, dependency, true, "downloading_yt_dlp", None, None);
    let client = Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|err| err.to_string())?;
    download_file(&client, download_url, &target, |downloaded, total| {
        if let Some(total) = total {
            if total > 0 {
                let ratio = (downloaded as f64 / total as f64).clamp(0.0, 1.0);
                let progress = (20.0 + ratio * 60.0).round() as i32;
                set_dependency_status(
                    app,
                    dependency,
                    true,
                    "downloading_yt_dlp",
                    Some(progress),
                    None,
                );
                return;
            }
        }
        set_dependency_status(app, dependency, true, "downloading_yt_dlp", None, None);
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&target, fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

/// Ensures ffmpeg is available, installing it automatically on Windows if not found.
pub fn ensure_ffmpeg_available(
    app: &AppHandle,
    dependency: &Arc<Mutex<DependencyRuntimeState>>,
) -> Result<(), String> {
    set_dependency_status(app, dependency, true, "checking_ffmpeg", Some(86), None);
    let ffmpeg = resolve_executable(app, "ffmpeg");
    let result = run_command_capture(app, &ffmpeg, &["-version"], DIAGNOSTICS_COMMAND_TIMEOUT_MS);
    if result.code == 0 {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        install_ffmpeg_windows(app, dependency)?;
        set_dependency_status(app, dependency, true, "checking_ffmpeg", Some(98), None);
        let ffmpeg = resolve_executable(app, "ffmpeg");
        let recheck =
            run_command_capture(app, &ffmpeg, &["-version"], DIAGNOSTICS_COMMAND_TIMEOUT_MS);
        if recheck.code == 0 {
            return Ok(());
        }
        return Err("ffmpeg 자동 설치 후에도 실행에 실패했습니다.".to_string());
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("ffmpeg를 찾지 못했습니다. 시스템에 설치 후 다시 시도해 주세요.".to_string())
    }
}

/// Installs ffmpeg on Windows by downloading the official zip and extracting the binaries.
#[cfg(target_os = "windows")]
pub fn install_ffmpeg_windows(
    app: &AppHandle,
    dependency: &Arc<Mutex<DependencyRuntimeState>>,
) -> Result<(), String> {
    use crate::file_ops::app_data_dir;
    use zip::ZipArchive;

    set_dependency_status(app, dependency, true, "installing_ffmpeg", Some(88), None);

    let managed_dir = managed_bin_dir_path(app);
    fs::create_dir_all(&managed_dir).map_err(|err| err.to_string())?;
    let ffmpeg_target = managed_executable_path(app, "ffmpeg");
    let ffprobe_target = managed_executable_path(app, "ffprobe");
    let zip_path = app_data_dir(app).join("ffmpeg-windows.zip");

    let client = Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|err| err.to_string())?;
    download_file(
        &client,
        FFMPEG_DOWNLOAD_URL_WINDOWS,
        &zip_path,
        |downloaded, total| {
            if let Some(total) = total {
                if total > 0 {
                    let ratio = (downloaded as f64 / total as f64).clamp(0.0, 1.0);
                    let progress = (88.0 + ratio * 8.0).round() as i32;
                    set_dependency_status(
                        app,
                        dependency,
                        true,
                        "installing_ffmpeg",
                        Some(progress),
                        None,
                    );
                    return;
                }
            }
            set_dependency_status(app, dependency, true, "installing_ffmpeg", None, None);
        },
    )?;

    let zip_file = fs::File::open(&zip_path).map_err(|err| err.to_string())?;
    let mut archive = ZipArchive::new(zip_file).map_err(|err| err.to_string())?;
    let mut ffmpeg_installed = false;
    let mut ffprobe_installed = false;

    for idx in 0..archive.len() {
        let mut entry = archive.by_index(idx).map_err(|err| err.to_string())?;
        if !entry.is_file() {
            continue;
        }

        let entry_name = entry.name().replace('\\', "/");
        let destination = if entry_name.ends_with("/bin/ffmpeg.exe") {
            Some(ffmpeg_target.clone())
        } else if entry_name.ends_with("/bin/ffprobe.exe") {
            Some(ffprobe_target.clone())
        } else {
            None
        };

        let Some(destination) = destination else {
            continue;
        };

        let mut output = fs::File::create(&destination).map_err(|err| err.to_string())?;
        std::io::copy(&mut entry, &mut output).map_err(|err| err.to_string())?;

        if destination == ffmpeg_target {
            ffmpeg_installed = true;
        } else if destination == ffprobe_target {
            ffprobe_installed = true;
        }

        if ffmpeg_installed && ffprobe_installed {
            break;
        }
    }

    let _ = fs::remove_file(&zip_path);

    if !ffmpeg_installed {
        return Err("ffmpeg 자동 설치에 실패했습니다.".to_string());
    }

    Ok(())
}

/// Bootstraps all required dependencies (yt-dlp and ffmpeg).
pub fn bootstrap_dependencies(
    app: &AppHandle,
    dependency: &Arc<Mutex<DependencyRuntimeState>>,
) -> Result<(), String> {
    set_dependency_status(app, dependency, true, "preparing", Some(5), None);
    ensure_ytdlp(app, dependency, true)?;
    ensure_ffmpeg_available(app, dependency)?;
    set_dependency_status(app, dependency, false, "ready", Some(100), None);
    if let Ok(mut state) = dependency.lock() {
        state.dependencies_ready = true;
        state.in_progress = false;
    }
    Ok(())
}

/// Starts the dependency bootstrap in a background thread if it has not already started.
pub fn start_dependency_bootstrap_if_needed(
    app: AppHandle,
    dependency: Arc<Mutex<DependencyRuntimeState>>,
) {
    let should_start = {
        let mut state = match dependency.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        if state.dependencies_ready || state.in_progress {
            false
        } else {
            state.in_progress = true;
            true
        }
    };

    if !should_start {
        return;
    }

    std::thread::spawn(move || {
        if let Err(err) = bootstrap_dependencies(&app, &dependency) {
            set_dependency_status(&app, &dependency, false, "failed", None, Some(err.clone()));
            if let Ok(mut state) = dependency.lock() {
                state.dependencies_ready = false;
                state.in_progress = false;
                state.status.error_message = Some(err);
            }
        }
    });
}

/// Waits for the dependency bootstrap to complete, starting it if necessary.
///
/// Returns `Ok(())` when dependencies are ready, or `Err` if bootstrap fails
/// or the wait times out.
pub fn wait_for_dependencies(
    app: &AppHandle,
    dependency: &Arc<Mutex<DependencyRuntimeState>>,
) -> Result<(), String> {
    start_dependency_bootstrap_if_needed(app.clone(), dependency.clone());

    for _ in 0..600 {
        if let Ok(state) = dependency.lock() {
            if state.dependencies_ready {
                return Ok(());
            }
            if !state.in_progress && state.status.phase == "failed" {
                return Err(state
                    .status
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "dependency bootstrap failed".to_string()));
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    Err("의존성 준비 시간이 초과되었습니다.".to_string())
}

// ============================================================================
// Shared type alias for the Tauri state
// ============================================================================

/// Thread-safe shared handle for the dependency bootstrap state.
#[derive(Clone)]
pub struct SharedDependencyState(pub Arc<Mutex<DependencyRuntimeState>>);
