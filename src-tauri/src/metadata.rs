use crate::dependencies::{wait_for_dependencies, SharedDependencyState};
use crate::file_ops::{resolve_executable, run_command_capture};
use crate::types::CommandResult;
use crate::utils::normalize_youtube_video_url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use tauri::{AppHandle, State};

const ANALYZE_TIMEOUT_MS: u64 = 15_000;

// ============================================================================
// Domain types
// ============================================================================

/// Specifies whether the user wants a video or audio-only download.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadMode {
    Video,
    Audio,
}

/// A single selectable format option returned by URL analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityOption {
    pub id: String,
    pub label: String,
    pub ext: String,
    #[serde(rename = "type")]
    pub mode: DownloadMode,
}

/// Metadata and available quality options for a successfully analyzed URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    #[serde(rename = "sourceUrl")]
    pub source_url: String,
    pub title: String,
    pub channel: String,
    #[serde(rename = "durationSec")]
    pub duration_sec: i64,
    #[serde(rename = "thumbnailUrl")]
    pub thumbnail_url: String,
    #[serde(rename = "videoOptions")]
    pub video_options: Vec<QualityOption>,
    #[serde(rename = "audioOptions")]
    pub audio_options: Vec<QualityOption>,
}

// ============================================================================
// Private helpers
// ============================================================================

/// Runs yt-dlp with `-J` and returns the parsed JSON payload.
fn fetch_metadata_json(app: &AppHandle, url: &str) -> Result<Value, String> {
    let yt_dlp = resolve_executable(app, "yt-dlp");
    let output = run_command_capture(
        app,
        &yt_dlp,
        &["--no-playlist", "-J", "--no-warnings", url],
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

    serde_json::from_str(&output.stdout).map_err(|e| e.to_string())
}

/// Parses video and audio QualityOptions from a yt-dlp JSON payload.
fn parse_quality_options(json: &Value) -> (Vec<QualityOption>, Vec<QualityOption>) {
    let formats = json
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

    // Deduplicate video options by height
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

    let audio_options: Vec<QualityOption> = audio_candidates
        .into_iter()
        .map(|(_, option)| option)
        .collect();

    (video_options, audio_options)
}

/// Builds the final AnalysisResult from metadata JSON and parsed options.
fn build_analysis_result(
    json: &Value,
    normalized_url: String,
    video_options: Vec<QualityOption>,
    audio_options: Vec<QualityOption>,
) -> AnalysisResult {
    let video_options = if video_options.is_empty() {
        vec![QualityOption {
            id: "bestvideo+bestaudio".to_string(),
            label: "Best Video".to_string(),
            ext: "mp4".to_string(),
            mode: DownloadMode::Video,
        }]
    } else {
        video_options
    };

    let audio_options = if audio_options.is_empty() {
        vec![QualityOption {
            id: "bestaudio".to_string(),
            label: "Best Audio".to_string(),
            ext: "m4a".to_string(),
            mode: DownloadMode::Audio,
        }]
    } else {
        audio_options
    };

    AnalysisResult {
        source_url: normalized_url,
        title: json
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Unknown Title")
            .to_string(),
        channel: json
            .get("uploader")
            .and_then(Value::as_str)
            .unwrap_or("Unknown Channel")
            .to_string(),
        duration_sec: json.get("duration").and_then(Value::as_i64).unwrap_or(0),
        thumbnail_url: json
            .get("thumbnail")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        video_options,
        audio_options,
    }
}

// ============================================================================
// Tauri command
// ============================================================================

/// Fetches metadata for the given URL and returns available quality options.
#[tauri::command]
pub async fn analyze_url(
    app: AppHandle,
    dependency: State<'_, SharedDependencyState>,
    url: String,
) -> CommandResult<AnalysisResult> {
    let normalized_url = normalize_youtube_video_url(&url);
    if normalized_url.trim().is_empty() {
        return Err("URL is empty".to_string());
    }

    wait_for_dependencies(&app, &dependency.0)?;

    let payload = fetch_metadata_json(&app, normalized_url.trim())?;

    // Reject live streams
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

    let (video_options, audio_options) = parse_quality_options(&payload);
    Ok(build_analysis_result(
        &payload,
        normalized_url,
        video_options,
        audio_options,
    ))
}
