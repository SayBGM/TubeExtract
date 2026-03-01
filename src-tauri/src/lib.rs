use dirs::download_dir;
use fs2::statvfs;
use regex::Regex;
use reqwest::blocking::Client;
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, TryLockError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};
use url::Url;
use uuid::Uuid;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

type CommandResult<T> = Result<T, String>;

const MAX_LOG_LINES_PER_JOB: usize = 120;
const TEMP_DOWNLOADS_DIR: &str = "tmp-downloads";
const QUEUE_FILE: &str = "queue_state.json";
const SETTINGS_FILE: &str = "settings.json";
const RETRY_DELAY_TABLE_MS: [u64; 4] = [2000, 5000, 10000, 15000];
const MANAGED_BIN_DIR: &str = "bin";
const DIAGNOSTICS_COMMAND_TIMEOUT_MS: u64 = 10_000;
const ANALYZE_TIMEOUT_MS: u64 = 15_000;
const YTDLP_LATEST_API_URL: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
const YTDLP_VERSION_CHECK_TIMEOUT_MS: u64 = 5_000;
const YTDLP_DOWNLOAD_URL_WINDOWS: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const YTDLP_DOWNLOAD_URL_MACOS: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos";
const YTDLP_DOWNLOAD_URL_LINUX: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux";
#[cfg(target_os = "windows")]
const FFMPEG_DOWNLOAD_URL_WINDOWS: &str =
    "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip";

#[cfg(target_os = "windows")]
const COMMON_BINARY_DIRS: &[&str] = &[
    "C:\\Program Files\\yt-dlp",
    "C:\\Program Files\\ffmpeg\\bin",
    "C:\\Windows\\System32",
];

#[cfg(not(target_os = "windows"))]
const COMMON_BINARY_DIRS: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"];

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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsResult {
    yt_dlp_available: bool,
    ffmpeg_available: bool,
    download_path_writable: bool,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageStats {
    total_bytes: u64,
    available_bytes: u64,
    used_bytes: u64,
    used_percent: f64,
    download_dir_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateCheckResult {
    is_duplicate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    existing_output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyBootstrapStatus {
    in_progress: bool,
    phase: String,
    progress_percent: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_message: Option<String>,
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
}

#[derive(Debug, Clone)]
struct AppState {
    queue: Vec<QueueItem>,
    settings: AppSettings,
    active_job_id: Option<String>,
}

#[derive(Clone)]
struct SharedState(Arc<Mutex<AppState>>);

#[derive(Clone)]
struct ActiveProcess {
    job_id: String,
    child: Arc<Mutex<Child>>,
}

#[derive(Default)]
struct RuntimeState {
    active_process: Option<ActiveProcess>,
    // Shutdown sender for graceful worker thread termination.
    // None until a worker thread is started.
    shutdown_tx: Option<std::sync::mpsc::Sender<()>>,
    // Handle for the worker thread.
    worker_handle: Option<std::thread::JoinHandle<()>>,
}

#[derive(Clone)]
struct SharedRuntime(Arc<Mutex<RuntimeState>>);

#[derive(Clone)]
struct SharedDependencyState(Arc<Mutex<DependencyRuntimeState>>);

#[derive(Debug, Clone)]
struct DependencyRuntimeState {
    status: DependencyBootstrapStatus,
    dependencies_ready: bool,
    in_progress: bool,
}

fn default_settings() -> AppSettings {
    AppSettings {
        download_dir: download_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .to_string_lossy()
            .to_string(),
        max_retries: 3,
        language: "ko".to_string(),
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

fn default_dependency_status() -> DependencyBootstrapStatus {
    DependencyBootstrapStatus {
        in_progress: false,
        phase: "idle".to_string(),
        progress_percent: None,
        error_message: None,
    }
}

fn emit_dependency_status(app: &AppHandle, status: &DependencyBootstrapStatus) {
    let _ = app.emit(
        "dependency-bootstrap-updated",
        status.clone(),
    );
}

fn set_dependency_status(
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

fn app_data_dir(app: &AppHandle) -> PathBuf {
    let fallback = download_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tubeextract-data");
    let path = app.path().app_data_dir().unwrap_or(fallback);
    let _ = fs::create_dir_all(&path);
    path
}

fn temp_downloads_root_dir(app: &AppHandle) -> PathBuf {
    app_data_dir(app).join(TEMP_DOWNLOADS_DIR)
}

fn temp_job_dir_path(app: &AppHandle, job_id: &str) -> PathBuf {
    temp_downloads_root_dir(app).join(job_id)
}

fn queue_file_path(app: &AppHandle) -> PathBuf {
    app_data_dir(app).join(QUEUE_FILE)
}

fn settings_file_path(app: &AppHandle) -> PathBuf {
    app_data_dir(app).join(SETTINGS_FILE)
}

fn managed_bin_dir_path(app: &AppHandle) -> PathBuf {
    app_data_dir(app).join(MANAGED_BIN_DIR)
}

fn managed_executable_path(app: &AppHandle, binary_name: &str) -> PathBuf {
    let executable_name = if cfg!(target_os = "windows") {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    };
    managed_bin_dir_path(app).join(executable_name)
}

fn bundled_executable_path(app: &AppHandle, binary_name: &str) -> Option<PathBuf> {
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

fn binary_with_platform_extension(binary_name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    }
}

fn resolve_executable(app: &AppHandle, binary_name: &str) -> String {
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

fn managed_path_env(app: &AppHandle) -> String {
    let mut paths = Vec::new();
    paths.push(managed_bin_dir_path(app));
    if let Some(path_var) = env::var_os("PATH") {
        paths.extend(env::split_paths(&path_var));
    }
    env::join_paths(paths)
        .map(|joined| joined.to_string_lossy().to_string())
        .unwrap_or_else(|_| env::var("PATH").unwrap_or_default())
}

fn configure_hidden_process(command: &mut Command) -> &mut Command {
    #[cfg(target_os = "windows")]
    {
        // CREATE_NO_WINDOW
        command.creation_flags(0x08000000);
    }
    command
}

#[derive(Debug)]
struct CommandCaptureResult {
    code: i32,
    stdout: String,
    stderr: String,
    timed_out: bool,
}

fn run_command_capture(
    app: &AppHandle,
    command: &str,
    args: &[&str],
    timeout_ms: u64,
) -> CommandCaptureResult {
    let mut cmd = Command::new(command);
    configure_hidden_process(&mut cmd);
    let mut child = match cmd
        .args(args)
        .env("PATH", managed_path_env(app))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            return CommandCaptureResult {
                code: 1,
                stdout: String::new(),
                stderr: err.to_string(),
                timed_out: false,
            };
        }
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

fn normalize_ytdlp_version(input: &str) -> String {
    input.trim().trim_start_matches('v').to_string()
}

fn installed_ytdlp_version(app: &AppHandle, target: &Path) -> Option<String> {
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

fn latest_ytdlp_version() -> Option<String> {
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
        .json::<Value>()
        .ok()?;
    let tag = payload.get("tag_name").and_then(Value::as_str).unwrap_or_default();
    let version = normalize_ytdlp_version(tag);
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

fn download_file(
    client: &Client,
    url: &str,
    destination: &Path,
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
        file.write_all(&buffer[..read]).map_err(|err| err.to_string())?;
        downloaded = downloaded.saturating_add(read as u64);
        on_progress(downloaded, total);
    }
    Ok(())
}

fn ensure_ytdlp(
    app: &AppHandle,
    dependency: &Arc<Mutex<DependencyRuntimeState>>,
    check_latest: bool,
) -> Result<(), String> {
    set_dependency_status(app, dependency, true, "checking_yt_dlp", Some(15), None);

    let resolved = resolve_executable(app, "yt-dlp");
    let check_existing = run_command_capture(app, &resolved, &["--version"], YTDLP_VERSION_CHECK_TIMEOUT_MS);
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
                set_dependency_status(app, dependency, true, "downloading_yt_dlp", Some(progress), None);
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

fn ensure_ffmpeg_available(
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
        let recheck = run_command_capture(app, &ffmpeg, &["-version"], DIAGNOSTICS_COMMAND_TIMEOUT_MS);
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

#[cfg(target_os = "windows")]
fn install_ffmpeg_windows(
    app: &AppHandle,
    dependency: &Arc<Mutex<DependencyRuntimeState>>,
) -> Result<(), String> {
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
    download_file(&client, FFMPEG_DOWNLOAD_URL_WINDOWS, &zip_path, |downloaded, total| {
        if let Some(total) = total {
            if total > 0 {
                let ratio = (downloaded as f64 / total as f64).clamp(0.0, 1.0);
                let progress = (88.0 + ratio * 8.0).round() as i32;
                set_dependency_status(app, dependency, true, "installing_ffmpeg", Some(progress), None);
                return;
            }
        }
        set_dependency_status(app, dependency, true, "installing_ffmpeg", None, None);
    })?;

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

fn bootstrap_dependencies(app: &AppHandle, dependency: &Arc<Mutex<DependencyRuntimeState>>) -> Result<(), String> {
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

fn start_dependency_bootstrap_if_needed(app: AppHandle, dependency: Arc<Mutex<DependencyRuntimeState>>) {
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

fn wait_for_dependencies(app: &AppHandle, dependency: &Arc<Mutex<DependencyRuntimeState>>) -> Result<(), String> {
    start_dependency_bootstrap_if_needed(app.clone(), dependency.clone());

    for _ in 0..600 {
        if let Ok(state) = dependency.lock() {
            if state.dependencies_ready {
                return Ok(());
            }
            if !state.in_progress && state.status.phase == "failed" {
                return Err(
                    state
                        .status
                        .error_message
                        .clone()
                        .unwrap_or_else(|| "dependency bootstrap failed".to_string()),
                );
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    Err("의존성 준비 시간이 초과되었습니다.".to_string())
}

fn normalize_download_dir(raw_path: &str) -> String {
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

fn persist_queue(app: &AppHandle, state: &AppState) {
    let path = queue_file_path(app);
    if let Ok(serialized) = serde_json::to_string_pretty(&state.queue) {
        let _ = fs::write(path, serialized);
    }
}

fn persist_settings(app: &AppHandle, settings: &AppSettings) {
    let path = settings_file_path(app);
    if let Ok(serialized) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(path, serialized);
    }
}

fn load_settings(app: &AppHandle, state: &mut AppState) {
    let path = settings_file_path(app);
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    let Ok(parsed) = serde_json::from_str::<PersistedSettings>(&content) else {
        return;
    };
    if let Some(download_dir) = parsed.download_dir {
        state.settings.download_dir = normalize_download_dir(&download_dir);
    }
    if let Some(max_retries) = parsed.max_retries {
        state.settings.max_retries = max_retries.clamp(0, 10);
    }
    if let Some(language) = parsed.language {
        state.settings.language = language;
    }
}

fn load_queue(app: &AppHandle, state: &mut AppState) {
    let path = queue_file_path(app);
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    let Ok(mut parsed) = serde_json::from_str::<Vec<QueueItem>>(&content) else {
        return;
    };
    for item in &mut parsed {
        if item.status == "downloading" {
            item.status = "queued".to_string();
        }
        if item.download_log.is_none() {
            item.download_log = Some(Vec::new());
        }
    }
    state.queue = parsed;
}

fn retry_delay_ms(attempt: usize) -> u64 {
    let idx = attempt.min(RETRY_DELAY_TABLE_MS.len().saturating_sub(1));
    RETRY_DELAY_TABLE_MS[idx]
}

fn remove_directory_safe(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

fn move_file_with_fallback(source: &Path, destination: &Path) -> Result<(), String> {
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
            fs::copy(source, destination).map_err(|copy_err| copy_err.to_string())?;
            fs::remove_file(source).map_err(|remove_err| remove_err.to_string())?;
            Ok(())
        }
    }
}

fn resolve_downloaded_file_path(temp_dir: &Path, expected_ext: &str) -> Result<PathBuf, String> {
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

fn normalize_youtube_video_url(raw_url: &str) -> String {
    let input = raw_url.trim();
    if input.is_empty() {
        return input.to_string();
    }

    let parsed = Url::parse(input);
    if parsed.is_err() {
        return input.to_string();
    }
    let parsed = parsed.unwrap_or_else(|_| unreachable!());
    let host = parsed.host_str().unwrap_or_default().to_lowercase();

    if host.contains("youtube.com") {
        if let Some(video_id) = parsed
            .query_pairs()
            .find(|(k, _)| k == "v")
            .map(|(_, v)| v.to_string())
        {
            return format!("https://www.youtube.com/watch?v={video_id}");
        }
        let path_parts: Vec<&str> = parsed.path().split('/').filter(|part| !part.is_empty()).collect();
        if path_parts.len() >= 2 && (path_parts[0] == "shorts" || path_parts[0] == "live") {
            return format!("https://www.youtube.com/watch?v={}", path_parts[1]);
        }
    }

    if host == "youtu.be" {
        let video_id = parsed.path().split('/').find(|part| !part.is_empty());
        if let Some(video_id) = video_id {
            return format!("https://www.youtube.com/watch?v={video_id}");
        }
    }

    input.to_string()
}

fn sanitize_file_name(raw_name: &str) -> String {
    let re = Regex::new(r#"[\\/:*?"<>|]"#).unwrap_or_else(|_| unreachable!());
    let replaced = re.replace_all(raw_name, "_");
    let collapsed = replaced.split_whitespace().collect::<Vec<&str>>().join(" ");
    let trimmed = collapsed.trim().trim_end_matches(['.', ' ']).to_string();
    if trimmed.is_empty() {
        "download".to_string()
    } else {
        trimmed.chars().take(160).collect()
    }
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
                format!("{quality_id}+bestaudio/best")
            }
        }
    }
}

fn parse_progress_percent(line: &str) -> Option<f64> {
    let idx = line.find('%')?;
    let prefix = &line[..idx];
    let start = prefix.rfind(|ch: char| !(ch.is_ascii_digit() || ch == '.'))?;
    let value = prefix[start + 1..].trim().parse::<f64>().ok()?;
    Some(value)
}

fn parse_speed(line: &str) -> Option<String> {
    let at = line.find(" at ")?;
    let eta = line.find(" ETA")?;
    if eta <= at + 4 {
        return None;
    }
    Some(line[at + 4..eta].trim().to_string())
}

fn parse_eta(line: &str) -> Option<String> {
    let eta = line.find(" ETA ")?;
    Some(line[eta + 5..].trim().to_string())
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

fn handle_download_output_line(shared: &Arc<Mutex<AppState>>, app: &AppHandle, job_id: &str, line: &str) {
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
                && item.error_message.as_deref() != Some(normalized) {
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
    let child = runtime
        .lock()
        .ok()
        .and_then(|mut guard| guard.active_process.take())
        .map(|active| active.child);
    if let Some(child) = child {
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
        if guard
            .active_process
            .as_ref()
            .map(|active| active.job_id == job_id)
            .unwrap_or(false)
        {
            guard.active_process = None;
        }
    }
}

fn start_worker_if_needed(app: AppHandle, shared: Arc<Mutex<AppState>>, runtime: Arc<Mutex<RuntimeState>>) {
    let should_start = {
        let mut state = shared.lock().unwrap_or_else(|e| {
    eprintln!("[STABILITY] Mutex poisoned, recovering: {:?}", e);
    e.into_inner()
});
        if state.active_job_id.is_some() {
            false
        } else if state.queue.iter().any(|item| item.status == "queued") {
            state.active_job_id = Some("worker".to_string());
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
        let mut rt = runtime.lock().unwrap_or_else(|e| {
            eprintln!("[STABILITY] Runtime mutex poisoned, recovering: {:?}", e);
            e.into_inner()
        });
        rt.shutdown_tx = Some(shutdown_tx);
    }

    // Clone runtime Arc so the closure can own one reference and we keep another for handle storage.
    let runtime_for_thread = runtime.clone();
    let handle = std::thread::spawn(move || {
        let runtime = runtime_for_thread;
        loop {
        // Check for shutdown signal before processing the next job.
        if shutdown_rx.try_recv().is_ok() {
            eprintln!("[STABILITY] Worker thread received shutdown signal; exiting");
            return;
        }
        let current_job = {
            let mut state = shared.lock().unwrap_or_else(|e| {
    eprintln!("[STABILITY] Mutex poisoned, recovering: {:?}", e);
    e.into_inner()
});
            let next_index = state.queue.iter().position(|item| item.status == "queued");
            if let Some(index) = next_index {
                state.queue[index].status = "downloading".to_string();
                state.queue[index].progress_percent = 0.0;
                let job = state.queue[index].clone();
                emit_queue_updated(&app, &state);
                Some(job)
            } else {
                state.active_job_id = None;
                emit_queue_updated(&app, &state);
                None
            }
        };

        let Some(job) = current_job else {
            return;
        };

        if let Some(dependency) = app.try_state::<SharedDependencyState>() {
            if let Err(err) = wait_for_dependencies(&app, &dependency.0) {
                let mut state = shared.lock().unwrap_or_else(|e| {
    eprintln!("[STABILITY] Mutex poisoned, recovering: {:?}", e);
    e.into_inner()
});
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
            let state = shared.lock().unwrap_or_else(|e| {
    eprintln!("[STABILITY] Mutex poisoned, recovering: {:?}", e);
    e.into_inner()
});
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
                let state = shared.lock().unwrap_or_else(|e| {
    eprintln!("[STABILITY] Mutex poisoned, recovering: {:?}", e);
    e.into_inner()
});
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
                        let mut guard = runtime.lock().unwrap_or_else(|e| {
    eprintln!("[STABILITY] Runtime mutex poisoned, recovering: {:?}", e);
    e.into_inner()
});
                        guard.active_process = Some(ActiveProcess {
                            job_id: job.id.clone(),
                            child: child_arc.clone(),
                        });
                    }

                    let shared_stdout = shared.clone();
                    let app_stdout = app.clone();
                    let job_id_stdout = job.id.clone();
                    let stdout_thread = stdout_reader.map(|reader| {
                        std::thread::spawn(move || {
                            for line in reader.lines().map_while(Result::ok) {
                                handle_download_output_line(&shared_stdout, &app_stdout, &job_id_stdout, &line);
                            }
                        })
                    });

                    let shared_stderr = shared.clone();
                    let app_stderr = app.clone();
                    let job_id_stderr = job.id.clone();
                    let stderr_thread = stderr_reader.map(|reader| {
                        std::thread::spawn(move || {
                            for line in reader.lines().map_while(Result::ok) {
                                handle_download_output_line(&shared_stderr, &app_stderr, &job_id_stderr, &line);
                            }
                        })
                    });

                    let wait_result = loop {
                        let status = {
                            // Keep the child lock only for try_wait, so pause/cancel can acquire it.
                            let mut locked_child =
                                child_arc.lock().unwrap_or_else(|e| {
    eprintln!("[STABILITY] Child mutex poisoned, recovering: {:?}", e);
    e.into_inner()
});
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
            {
                let mut state = shared.lock().unwrap_or_else(|e| {
    eprintln!("[STABILITY] Mutex poisoned, recovering: {:?}", e);
    e.into_inner()
});
                if let Some(item) = state.queue.iter_mut().find(|item| item.id == job.id) {
                    if item.status == "paused" || item.status == "canceled" {
                        // Keep paused/canceled state as-is.
                    } else if process_ok {
                        let expected_ext = expected_extension(&job.mode);
                        let move_result = resolve_downloaded_file_path(&temp_dir, expected_ext)
                            .and_then(|completed_path| move_file_with_fallback(&completed_path, &final_output_path));
                        match move_result {
                            Ok(()) => {
                                item.status = "completed".to_string();
                                item.progress_percent = 100.0;
                                item.output_path = Some(final_output_path.to_string_lossy().to_string());
                                item.error_message = None;
                            }
                            Err(err) => {
                                item.status = "failed".to_string();
                                item.error_message = Some(err);
                            }
                        }
                    } else {
                        let fallback = process_error.unwrap_or_else(|| "다운로드 실패".to_string());
                        item.error_message = Some(fallback);
                        if attempt < max_retries {
                            should_retry = true;
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
                std::thread::sleep(Duration::from_millis(retry_delay_ms(attempt)));
                continue;
            }
            break;
        }

        remove_directory_safe(&temp_dir);
        } // end loop
    }); // end thread closure

    // Store the worker handle for graceful join on shutdown.
    {
        let mut rt = runtime.lock().unwrap_or_else(|e| {
            eprintln!("[STABILITY] Runtime mutex poisoned, recovering: {:?}", e);
            e.into_inner()
        });
        rt.worker_handle = Some(handle);
    }
}

#[tauri::command]
async fn analyze_url(app: AppHandle, dependency: State<'_, SharedDependencyState>, url: String) -> CommandResult<AnalysisResult> {
    let normalized_url = normalize_youtube_video_url(&url);
    if normalized_url.trim().is_empty() {
        return Err("URL is empty".to_string());
    }

    wait_for_dependencies(&app, &dependency.0)?;

    let yt_dlp = resolve_executable(&app, "yt-dlp");
    let output = run_command_capture(
        &app,
        &yt_dlp,
        &["--no-playlist", "-J", "--no-warnings", normalized_url.trim()],
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

    let mut audio_options: Vec<QualityOption> =
        audio_candidates.into_iter().map(|(_, option)| option).collect();
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
    let state = state.0.lock().map_err(|_| "state lock poisoned".to_string())?;
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

    let mut locked = state.0.lock().map_err(|_| "state lock poisoned".to_string())?;
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
    let mut state = state.0.lock().map_err(|_| "state lock poisoned".to_string())?;
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
    let mut state = state.0.lock().map_err(|_| "state lock poisoned".to_string())?;
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
async fn clear_terminal_jobs(app: AppHandle, state: State<'_, SharedState>) -> CommandResult<QueueSnapshot> {
    let mut state = state.0.lock().map_err(|_| "state lock poisoned".to_string())?;
    state
        .queue
        .retain(|item| item.status != "completed" && item.status != "failed" && item.status != "canceled");
    let snapshot = queue_snapshot(&state);
    emit_queue_updated(&app, &state);
    persist_queue(&app, &state);
    Ok(snapshot)
}

#[tauri::command]
async fn get_queue_snapshot(state: State<'_, SharedState>) -> CommandResult<QueueSnapshot> {
    let state = state.0.lock().map_err(|_| "state lock poisoned".to_string())?;
    Ok(queue_snapshot(&state))
}

#[tauri::command]
async fn get_settings(state: State<'_, SharedState>) -> CommandResult<AppSettings> {
    let state = state.0.lock().map_err(|_| "state lock poisoned".to_string())?;
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
async fn set_settings(app: AppHandle, state: State<'_, SharedState>, settings: AppSettings) -> CommandResult<()> {
    let mut state = state.0.lock().map_err(|_| "state lock poisoned".to_string())?;
    state.settings = AppSettings {
        download_dir: normalize_download_dir(&settings.download_dir),
        max_retries: settings.max_retries.clamp(0, 10),
        language: settings.language,
    };
    persist_settings(&app, &state.settings);
    Ok(())
}

#[tauri::command]
async fn run_diagnostics(
    app: AppHandle,
    state: State<'_, SharedState>,
    dependency: State<'_, SharedDependencyState>,
) -> CommandResult<DiagnosticsResult> {
    let dependency_error_message = wait_for_dependencies(&app, &dependency.0).err();
    let state = state.0.lock().map_err(|_| "state lock poisoned".to_string())?;
    let yt_dlp = resolve_executable(&app, "yt-dlp");
    let ffmpeg = resolve_executable(&app, "ffmpeg");
    let yt = run_command_capture(&app, &yt_dlp, &["--version"], DIAGNOSTICS_COMMAND_TIMEOUT_MS);
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
        message: format!(
            "yt-dlp: {}{}{}, ffmpeg: {}{}{}, download-dir writable: {}{}",
            if yt_ok { "OK" } else { "FAIL" },
            if yt_ok {
                "".to_string()
            } else {
                format!(" ({})", truncate_reason(&yt_reason))
            },
            format!(" ({yt_dlp})"),
            if ff_ok { "OK" } else { "FAIL" },
            if ff_ok {
                "".to_string()
            } else {
                format!(" ({})", truncate_reason(&ff_reason))
            },
            format!(" ({ffmpeg})"),
            if writable { "OK" } else { "FAIL" }
            ,
            dependency_error_message
                .map(|message| format!(", bootstrap: {message}"))
                .unwrap_or_default()
        ),
    })
}

fn can_write_to_dir(dir: &Path) -> bool {
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

fn truncate_reason(reason: &str) -> String {
    reason.chars().take(120).collect()
}

fn calculate_directory_size(path: &Path) -> u64 {
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

#[tauri::command]
async fn check_update() -> CommandResult<Value> {
    Ok(serde_json::json!({
      "hasUpdate": false,
      "latestVersion": Value::Null,
      "url": Value::Null
    }))
}

#[tauri::command]
async fn get_storage_stats(state: State<'_, SharedState>) -> CommandResult<StorageStats> {
    let state = state.0.lock().map_err(|_| "state lock poisoned".to_string())?;
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

#[tauri::command]
async fn delete_file(app: AppHandle, state: State<'_, SharedState>, path: String) -> CommandResult<QueueSnapshot> {
    let mut state = state.0.lock().map_err(|_| "state lock poisoned".to_string())?;
    state
        .queue
        .retain(|item| item.output_path.as_ref().map(|p| p != &path).unwrap_or(true));
    let snapshot = queue_snapshot(&state);
    emit_queue_updated(&app, &state);
    persist_queue(&app, &state);
    Ok(snapshot)
}

#[tauri::command]
async fn open_folder(path: String) -> CommandResult<()> {
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

    #[cfg(target_os = "macos")]
    {
        if normalized.exists() && normalized.is_file() {
            Command::new("open")
                .arg("-R")
                .arg(&normalized)
                .spawn()
                .map_err(|err| err.to_string())?;
        } else {
            Command::new("open")
                .arg(&target)
                .spawn()
                .map_err(|err| err.to_string())?;
        }
    }
    #[cfg(target_os = "windows")]
    {
        if normalized.exists() && normalized.is_file() {
            let selected = fs::canonicalize(&normalized).unwrap_or_else(|_| normalized.clone());
            Command::new("explorer")
                .arg("/select,")
                .arg(selected)
                .spawn()
                .map_err(|err| err.to_string())?;
        } else {
            Command::new("explorer")
                .arg(target)
                .spawn()
                .map_err(|err| err.to_string())?;
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&target)
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn open_external_url(url: String) -> CommandResult<()> {
    if url.trim().is_empty() {
        return Ok(());
    }
    let parsed = Url::parse(url.trim()).map_err(|_| "유효한 URL이 아닙니다.".to_string())?;
    let scheme = parsed.scheme().to_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err("http/https URL만 열 수 있습니다.".to_string());
    }

    let target = parsed.to_string();
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&target)
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", &target])
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&target)
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub fn run() {
    let builder = tauri::Builder::default()
        .setup(|app| {
            remove_directory_safe(&temp_downloads_root_dir(app.app_handle()));

            let mut initial_state = AppState {
                queue: Vec::new(),
                settings: default_settings(),
                active_job_id: None,
            };
            load_settings(app.app_handle(), &mut initial_state);
            load_queue(app.app_handle(), &mut initial_state);
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
                // Send shutdown signal to worker thread when the window is destroyed.
                let app = window.app_handle();
                if let Some(runtime) = app.try_state::<SharedRuntime>() {
                    if let Ok(mut rt) = runtime.0.lock() {
                        if let Some(tx) = rt.shutdown_tx.take() {
                            let _ = tx.send(());
                            eprintln!("[STABILITY] Sent shutdown signal to worker thread");
                        }
                    }
                }
            }
        });

    if let Err(e) = builder.run(tauri::generate_context!()) {
        eprintln!("[FATAL] Tauri initialization failed: {:?}", e);
        std::process::exit(1);
    }
}
