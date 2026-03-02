use dirs::download_dir;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const TEMP_DOWNLOADS_DIR: &str = "tmp-downloads";
const QUEUE_FILE: &str = "queue_state.json";
const SETTINGS_FILE: &str = "settings.json";
const MANAGED_BIN_DIR: &str = "bin";

/// Well-known directories to search for yt-dlp and ffmpeg on Windows.
#[cfg(target_os = "windows")]
pub const COMMON_BINARY_DIRS: &[&str] = &[
    "C:\\Program Files\\yt-dlp",
    "C:\\Program Files\\ffmpeg\\bin",
    "C:\\Windows\\System32",
];

/// Well-known directories to search for yt-dlp and ffmpeg on non-Windows platforms.
#[cfg(not(target_os = "windows"))]
pub const COMMON_BINARY_DIRS: &[&str] =
    &["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"];

// ============================================================================
// Application directory helpers
// ============================================================================

/// Returns the application data directory, creating it if needed.
// @MX:ANCHOR: [AUTO] Root data directory for all persistent app files. fan_in=6.
// @MX:REASON: [AUTO] Used by queue_file_path, settings_file_path, managed_bin_dir_path, temp_downloads_root_dir, write_atomic, and diagnostics.
pub fn app_data_dir(app: &AppHandle) -> PathBuf {
    let fallback = download_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tubeextract-data");
    let path = app.path().app_data_dir().unwrap_or(fallback);
    let _ = fs::create_dir_all(&path);
    path
}

/// Returns the root directory for temporary downloads.
pub fn temp_downloads_root_dir(app: &AppHandle) -> PathBuf {
    app_data_dir(app).join(TEMP_DOWNLOADS_DIR)
}

/// Returns the temporary directory path for a specific download job.
pub fn temp_job_dir_path(app: &AppHandle, job_id: &str) -> PathBuf {
    temp_downloads_root_dir(app).join(job_id)
}

/// Returns the path to the persisted queue file.
pub fn queue_file_path(app: &AppHandle) -> PathBuf {
    app_data_dir(app).join(QUEUE_FILE)
}

/// Returns the path to the persisted settings file.
pub fn settings_file_path(app: &AppHandle) -> PathBuf {
    app_data_dir(app).join(SETTINGS_FILE)
}

/// Returns the directory where managed (downloaded) binaries are stored.
pub fn managed_bin_dir_path(app: &AppHandle) -> PathBuf {
    app_data_dir(app).join(MANAGED_BIN_DIR)
}

// ============================================================================
// Executable resolution helpers
// ============================================================================

/// Returns the path to a managed executable, with platform-appropriate extension.
pub fn managed_executable_path(app: &AppHandle, binary_name: &str) -> PathBuf {
    let executable_name = if cfg!(target_os = "windows") {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    };
    managed_bin_dir_path(app).join(executable_name)
}

/// Returns the path to a bundled executable in the app resource directory, if present.
pub fn bundled_executable_path(app: &AppHandle, binary_name: &str) -> Option<PathBuf> {
    let executable_name = if cfg!(target_os = "windows") {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    };
    let resource_dir = app.path().resource_dir().ok()?;
    let candidate = resource_dir.join("bin").join(executable_name);
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

/// Returns the binary name with the platform-appropriate extension.
pub fn binary_with_platform_extension(binary_name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    }
}

/// Resolves the full path of an executable, checking managed, bundled, PATH, and
/// well-known locations in order.
// @MX:ANCHOR: [AUTO] Single entry point for locating yt-dlp/ffmpeg executables. fan_in=5.
// @MX:REASON: [AUTO] Called by ensure_ytdlp, ensure_ffmpeg_available, run_diagnostics, and download worker.
pub fn resolve_executable(app: &AppHandle, binary_name: &str) -> String {
    let managed_path = managed_executable_path(app, binary_name);
    if managed_path.exists() {
        return managed_path.to_string_lossy().to_string();
    }
    if let Some(bundled_path) = bundled_executable_path(app, binary_name) {
        return bundled_path.to_string_lossy().to_string();
    }

    let with_ext = binary_with_platform_extension(binary_name);
    if let Some(path_var) = env::var_os("PATH") {
        for entry in env::split_paths(&path_var) {
            let candidate = entry.join(&with_ext);
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
    }

    for base_dir in COMMON_BINARY_DIRS {
        let candidate = PathBuf::from(base_dir).join(&with_ext);
        if candidate.exists() {
            return candidate.to_string_lossy().to_string();
        }
    }

    with_ext
}

/// Returns the PATH environment variable with the managed binary directory prepended.
pub fn managed_path_env(app: &AppHandle) -> String {
    let mut paths = Vec::new();
    paths.push(managed_bin_dir_path(app));
    if let Some(path_var) = env::var_os("PATH") {
        paths.extend(env::split_paths(&path_var));
    }
    env::join_paths(paths)
        .map(|joined| joined.to_string_lossy().to_string())
        .unwrap_or_else(|_| env::var("PATH").unwrap_or_default())
}

/// Configures a `Command` to run without a visible window on Windows.
pub fn configure_hidden_process(command: &mut Command) -> &mut Command {
    #[cfg(target_os = "windows")]
    {
        // CREATE_NO_WINDOW
        command.creation_flags(0x08000000);
    }
    command
}

// ============================================================================
// Command execution
// ============================================================================

/// Result of capturing a subprocess's output.
#[derive(Debug)]
pub struct CommandCaptureResult {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Builds and spawns a subprocess with the managed PATH and hidden-window flag.
///
/// Returns the spawned `Child` on success, or a `CommandCaptureResult` error struct on failure.
pub fn build_and_spawn_command(
    app: &AppHandle,
    command: &str,
    args: &[&str],
) -> Result<Child, CommandCaptureResult> {
    let mut cmd = Command::new(command);
    configure_hidden_process(&mut cmd);
    cmd.args(args)
        .env("PATH", managed_path_env(app))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| CommandCaptureResult {
            code: 1,
            stdout: String::new(),
            stderr: err.to_string(),
            timed_out: false,
        })
}

/// Runs a subprocess with an optional timeout watchdog, collecting its stdout/stderr.
///
/// If `timeout_ms > 0`, spawns a watchdog thread that kills the process after the deadline.
// @MX:ANCHOR: [AUTO] Central subprocess runner. fan_in=8.
// @MX:REASON: [AUTO] Used by installed_ytdlp_version, ensure_ytdlp, ensure_ffmpeg_available, run_diagnostics, analyze_url, download worker, and more.
pub fn run_command_capture(
    app: &AppHandle,
    command: &str,
    args: &[&str],
    timeout_ms: u64,
) -> CommandCaptureResult {
    let mut child = match build_and_spawn_command(app, command, args) {
        Ok(child) => child,
        Err(err_result) => return err_result,
    };

    // Watchdog: kill the process if timeout_ms > 0 and time is exceeded.
    if timeout_ms > 0 {
        let child_id = child.id();
        let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
        std::thread::spawn(move || {
            // Poll until deadline, then attempt to kill.
            while std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(100));
            }
            // Best-effort kill by PID; the process may have already exited.
            #[cfg(unix)]
            {
                let _ = Command::new("kill")
                    .args(["-9", &child_id.to_string()])
                    .status();
            }
            #[cfg(windows)]
            {
                let _ = Command::new("taskkill")
                    .args(["/F", "/PID", &child_id.to_string()])
                    .status();
            }
        });
    }

    // Poll for process completion.
    let deadline = if timeout_ms > 0 {
        Some(std::time::Instant::now() + Duration::from_millis(timeout_ms))
    } else {
        None
    };

    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if let Some(dl) = deadline {
                    if std::time::Instant::now() >= dl {
                        // Timed out; collect whatever output is available and return.
                        let stdout_bytes = child
                            .stdout
                            .take()
                            .and_then(|mut s| {
                                let mut buf = Vec::new();
                                s.read_to_end(&mut buf).ok().map(|_| buf)
                            })
                            .unwrap_or_default();
                        let stderr_bytes = child
                            .stderr
                            .take()
                            .and_then(|mut s| {
                                let mut buf = Vec::new();
                                s.read_to_end(&mut buf).ok().map(|_| buf)
                            })
                            .unwrap_or_default();
                        eprintln!("[STABILITY] Command timed out after {}ms", timeout_ms);
                        return CommandCaptureResult {
                            code: -1,
                            stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
                            stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
                            timed_out: true,
                        };
                    }
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                return CommandCaptureResult {
                    code: 1,
                    stdout: String::new(),
                    stderr: err.to_string(),
                    timed_out: false,
                };
            }
        }
    }

    // Process finished within timeout; collect output.
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(err) => {
            return CommandCaptureResult {
                code: 1,
                stdout: String::new(),
                stderr: err.to_string(),
                timed_out: false,
            };
        }
    };

    CommandCaptureResult {
        code: if output.status.success() {
            0
        } else {
            output.status.code().unwrap_or(1)
        },
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        timed_out: false,
    }
}

// ============================================================================
// Atomic file I/O
// ============================================================================

/// Atomically writes `content` to `path` using a temp-file + rename strategy.
/// On POSIX systems the rename is atomic. On Windows it is near-atomic.
// @MX:NOTE: [AUTO] Atomic write via temp-file + rename. POSIX atomic; near-atomic on Windows.
pub fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    let tmp_path = PathBuf::from(format!("{}.tmp", path.display()));
    fs::write(&tmp_path, content).map_err(|e| e.to_string())?;
    fs::rename(&tmp_path, path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        e.to_string()
    })?;
    Ok(())
}

// ============================================================================
// Directory / file helpers
// ============================================================================

/// Silently removes a directory and all its contents, ignoring errors.
pub fn remove_directory_safe(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

/// Moves a file atomically.
/// Same-FS: uses fs::rename (atomic). Cross-device: writes .incomplete marker,
/// copies, verifies size, removes marker, removes source.
// @MX:WARN: [AUTO] Cross-device copy is NOT atomic. Incomplete marker guards against power loss corruption.
// @MX:REASON: [AUTO] See SPEC-STABILITY-002 REQ-002 for incomplete marker protocol.
pub fn move_file_atomic(source: &Path, destination: &Path) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(err) => {
            let is_cross_device = err.kind() == std::io::ErrorKind::CrossesDevices
                || err.raw_os_error() == Some(18)
                || err.raw_os_error() == Some(17);
            if !is_cross_device {
                return Err(err.to_string());
            }
            // Cross-device: use .incomplete marker for crash safety
            let incomplete_path = PathBuf::from(format!("{}.incomplete", destination.display()));
            // Write marker (content is minimal JSON for diagnostics)
            let started_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let marker_content = serde_json::json!({
                "started_at": started_at,
                "source": source.display().to_string()
            });
            {
                let mut f = fs::File::create(&incomplete_path)
                    .map_err(|e| format!("Failed to create .incomplete marker: {e}"))?;
                f.write_all(marker_content.to_string().as_bytes())
                    .map_err(|e| e.to_string())?;
            }

            // Copy source → destination
            let copied_bytes = fs::copy(source, destination).map_err(|copy_err| {
                // Leave .incomplete marker so startup scan can detect it
                format!("Copy failed: {copy_err}")
            })?;

            // Verify size matches
            let source_size = fs::metadata(source).map(|m| m.len()).unwrap_or(0);
            if source_size > 0 && copied_bytes != source_size {
                return Err(format!(
                    "Size mismatch after copy: expected {source_size} bytes, got {copied_bytes}"
                ));
            }

            // Remove .incomplete marker (transfer complete)
            let _ = fs::remove_file(&incomplete_path);

            // Remove source
            fs::remove_file(source).map_err(|remove_err| remove_err.to_string())?;
            Ok(())
        }
    }
}

/// Resolves the path of a downloaded media file in a temporary directory.
///
/// Prefers `media.<ext>` if it exists; otherwise returns the most recently
/// modified file with a matching extension.
pub fn resolve_downloaded_file_path(
    temp_dir: &Path,
    expected_ext: &str,
) -> Result<PathBuf, String> {
    let prioritized = temp_dir.join(format!("media.{expected_ext}"));
    if prioritized.exists() {
        return Ok(prioritized);
    }

    let mut candidates: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
    let entries = fs::read_dir(temp_dir).map_err(|err| err.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .map(|value| value.to_string_lossy().to_string().to_lowercase())
            .unwrap_or_default();
        if ext != expected_ext.to_lowercase() {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        candidates.push((path, modified));
    }

    if candidates.is_empty() {
        return Err("완성 파일을 임시 폴더에서 찾지 못했습니다.".to_string());
    }

    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    Ok(candidates[0].0.clone())
}

/// Normalizes a raw download directory path.
///
/// - Empty string → system Downloads folder
/// - Absolute path → returned as-is
/// - `Users/...` on non-Windows → prefixed with `/`
/// - Anything else → returned as-is
pub fn normalize_download_dir(raw_path: &str) -> String {
    let raw = raw_path.trim();
    if raw.is_empty() {
        return download_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .to_string_lossy()
            .to_string();
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return raw.to_string();
    }
    if cfg!(not(target_os = "windows")) && raw.starts_with("Users/") {
        return format!("/{raw}");
    }
    raw.to_string()
}
