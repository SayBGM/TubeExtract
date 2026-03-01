use regex::Regex;
use url::Url;

/// Normalizes a YouTube URL to canonical watch format.
///
/// Handles:
/// - `youtube.com/watch?v=ID` → returned as-is
/// - `youtu.be/ID` → `youtube.com/watch?v=ID`
/// - `youtube.com/shorts/ID` → `youtube.com/watch?v=ID`
/// - `youtube.com/live/ID` → `youtube.com/watch?v=ID`
///
/// Non-YouTube URLs and invalid URLs are returned unchanged.
pub fn normalize_youtube_video_url(raw_url: &str) -> String {
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
        let path_parts: Vec<&str> = parsed
            .path()
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
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

/// Sanitizes a file name by replacing forbidden characters with underscores,
/// collapsing whitespace, trimming trailing dots/spaces, and truncating to 160 chars.
///
/// Returns `"download"` if the result would be empty.
pub fn sanitize_file_name(raw_name: &str) -> String {
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

/// Extracts the download progress percentage from a yt-dlp output line.
///
/// Returns `Some(f64)` when a percentage value is found before `%`,
/// otherwise returns `None`.
pub fn parse_progress_percent(line: &str) -> Option<f64> {
    let idx = line.find('%')?;
    let prefix = &line[..idx];
    let start = prefix.rfind(|ch: char| !(ch.is_ascii_digit() || ch == '.'))?;
    let value = prefix[start + 1..].trim().parse::<f64>().ok()?;
    Some(value)
}

/// Extracts the download speed from a yt-dlp output line.
///
/// Looks for the pattern ` at <speed> ETA` and returns the speed portion.
/// Returns `None` if the expected markers are not found in the correct order.
pub fn parse_speed(line: &str) -> Option<String> {
    let at = line.find(" at ")?;
    let eta = line.find(" ETA")?;
    if eta <= at + 4 {
        return None;
    }
    Some(line[at + 4..eta].trim().to_string())
}

/// Extracts the ETA value from a yt-dlp output line.
///
/// Looks for ` ETA ` marker and returns everything after it (trimmed).
/// Returns `None` if the marker is not found.
pub fn parse_eta(line: &str) -> Option<String> {
    let eta = line.find(" ETA ")?;
    Some(line[eta + 5..].trim().to_string())
}
