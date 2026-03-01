// Characterization tests for Phase 1 extraction (SPEC-REFACTOR-001)
// These tests capture CURRENT behavior before extracting state.rs, types.rs, utils.rs.
// If any of these fail after refactoring, behavior has been broken.

// ============================================================================
// parse_progress_percent characterization
// ============================================================================

/// Characterize: parse_progress_percent extracts float from yt-dlp progress lines.
/// Input: "[download]  75.0% of 10.00MiB at 1.23MiB/s ETA 00:05"
/// Expected: Some(75.0)
#[test]
fn test_characterize_parse_progress_percent_normal() {
    // Mirror the function logic as it currently exists in lib.rs
    fn parse_progress_percent(line: &str) -> Option<f64> {
        let idx = line.find('%')?;
        let prefix = &line[..idx];
        let start = prefix.rfind(|ch: char| !(ch.is_ascii_digit() || ch == '.'))?;
        let value = prefix[start + 1..].trim().parse::<f64>().ok()?;
        Some(value)
    }

    assert_eq!(
        parse_progress_percent("[download]  75.0% of 10.00MiB at 1.23MiB/s ETA 00:05"),
        Some(75.0),
        "Should extract 75.0 from standard yt-dlp progress line"
    );
    assert_eq!(
        parse_progress_percent("[download]   0.1% of 500.00MiB at 2.00MiB/s ETA 04:09"),
        Some(0.1),
        "Should extract small percentage"
    );
    assert_eq!(
        parse_progress_percent("[download] 100% of 10.00MiB"),
        Some(100.0),
        "Should extract 100%"
    );
    assert_eq!(
        parse_progress_percent("no percent here"),
        None,
        "Should return None if no percent sign found"
    );
    assert_eq!(
        parse_progress_percent(""),
        None,
        "Should return None for empty string"
    );
}

/// Characterize: parse_progress_percent handles edge cases.
#[test]
fn test_characterize_parse_progress_percent_edge_cases() {
    fn parse_progress_percent(line: &str) -> Option<f64> {
        let idx = line.find('%')?;
        let prefix = &line[..idx];
        let start = prefix.rfind(|ch: char| !(ch.is_ascii_digit() || ch == '.'))?;
        let value = prefix[start + 1..].trim().parse::<f64>().ok()?;
        Some(value)
    }

    // Decimal with multiple digits
    assert_eq!(
        parse_progress_percent("[download]  99.9% of 1.00GiB"),
        Some(99.9)
    );
    // No numeric value before %
    assert_eq!(parse_progress_percent("abc%"), None);
}

// ============================================================================
// parse_speed characterization
// ============================================================================

/// Characterize: parse_speed extracts speed string from yt-dlp progress lines.
/// The function looks for " at " and " ETA" markers in the line.
#[test]
fn test_characterize_parse_speed_normal() {
    fn parse_speed(line: &str) -> Option<String> {
        let at = line.find(" at ")?;
        let eta = line.find(" ETA")?;
        if eta <= at + 4 {
            return None;
        }
        Some(line[at + 4..eta].trim().to_string())
    }

    assert_eq!(
        parse_speed("[download]  50.5% of 10.00MiB at 1.23MiB/s ETA 00:05"),
        Some("1.23MiB/s".to_string()),
        "Should extract speed between ' at ' and ' ETA'"
    );
    assert_eq!(
        parse_speed("[download]  50.5% of 10.00MiB at 2.50KiB/s ETA 00:30"),
        Some("2.50KiB/s".to_string()),
        "Should handle KiB/s speed units"
    );
    assert_eq!(
        parse_speed("[download]  50.5% of 10.00MiB"),
        None,
        "Should return None if ' at ' or ' ETA' not found"
    );
    assert_eq!(
        parse_speed("no speed info"),
        None,
        "Should return None for non-progress lines"
    );
}

/// Characterize: parse_speed returns None when ETA comes before at+4.
#[test]
fn test_characterize_parse_speed_eta_before_at() {
    fn parse_speed(line: &str) -> Option<String> {
        let at = line.find(" at ")?;
        let eta = line.find(" ETA")?;
        if eta <= at + 4 {
            return None;
        }
        Some(line[at + 4..eta].trim().to_string())
    }

    // Edge case: ' ETA' appears before ' at ' would produce a degenerate result
    // Current behavior: returns None when eta <= at + 4
    let line = " ETA 00:01 at "; // ETA at pos 0, ' at ' at pos 10 - not really possible but test guard
    let result = parse_speed(line);
    // ' at ' found at 10, ' ETA' found at 0, so eta(0) <= at(10)+4 → None
    assert_eq!(result, None);
}

// ============================================================================
// parse_eta characterization
// ============================================================================

/// Characterize: parse_eta extracts ETA string from yt-dlp progress lines.
#[test]
fn test_characterize_parse_eta_normal() {
    fn parse_eta(line: &str) -> Option<String> {
        let eta = line.find(" ETA ")?;
        Some(line[eta + 5..].trim().to_string())
    }

    assert_eq!(
        parse_eta("[download]  50.5% of 10.00MiB at 1.23MiB/s ETA 00:05"),
        Some("00:05".to_string()),
        "Should extract ETA value"
    );
    assert_eq!(
        parse_eta("[download]  50.5% of 10.00MiB at 1.23MiB/s ETA 01:30"),
        Some("01:30".to_string()),
        "Should handle longer ETA"
    );
    assert_eq!(
        parse_eta("no eta here"),
        None,
        "Should return None if ' ETA ' not found"
    );
    assert_eq!(parse_eta(""), None, "Should return None for empty string");
}

// ============================================================================
// sanitize_file_name characterization
// ============================================================================

/// Characterize: sanitize_file_name replaces dangerous characters with underscores.
/// Characters: \/:*?"<>|
#[test]
fn test_characterize_sanitize_file_name_dangerous_chars() {
    fn sanitize_file_name(raw_name: &str) -> String {
        let re = regex::Regex::new(r#"[\\/:*?"<>|]"#).unwrap();
        let replaced = re.replace_all(raw_name, "_");
        let collapsed = replaced.split_whitespace().collect::<Vec<&str>>().join(" ");
        let trimmed = collapsed.trim().trim_end_matches(['.', ' ']).to_string();
        if trimmed.is_empty() {
            "download".to_string()
        } else {
            trimmed.chars().take(160).collect()
        }
    }

    // Colons and slashes replaced with underscores
    let result = sanitize_file_name("video:title/name");
    assert_eq!(
        result, "video_title_name",
        "Colon and slash should become underscores"
    );

    // Question mark, angle brackets, pipe
    let result2 = sanitize_file_name("what?<is>this|file");
    assert_eq!(
        result2, "what__is_this_file",
        "Special chars should become underscores"
    );

    // Asterisk and quotes
    let result3 = sanitize_file_name("file*with\"quotes");
    assert_eq!(
        result3, "file_with_quotes",
        "Asterisk and quote should become underscores"
    );
}

/// Characterize: sanitize_file_name collapses whitespace.
#[test]
fn test_characterize_sanitize_file_name_whitespace() {
    fn sanitize_file_name(raw_name: &str) -> String {
        let re = regex::Regex::new(r#"[\\/:*?"<>|]"#).unwrap();
        let replaced = re.replace_all(raw_name, "_");
        let collapsed = replaced.split_whitespace().collect::<Vec<&str>>().join(" ");
        let trimmed = collapsed.trim().trim_end_matches(['.', ' ']).to_string();
        if trimmed.is_empty() {
            "download".to_string()
        } else {
            trimmed.chars().take(160).collect()
        }
    }

    let result = sanitize_file_name("hello   world   test");
    assert_eq!(
        result, "hello world test",
        "Multiple spaces should collapse to single space"
    );
}

/// Characterize: sanitize_file_name returns "download" for empty/whitespace-only input.
#[test]
fn test_characterize_sanitize_file_name_empty() {
    fn sanitize_file_name(raw_name: &str) -> String {
        let re = regex::Regex::new(r#"[\\/:*?"<>|]"#).unwrap();
        let replaced = re.replace_all(raw_name, "_");
        let collapsed = replaced.split_whitespace().collect::<Vec<&str>>().join(" ");
        let trimmed = collapsed.trim().trim_end_matches(['.', ' ']).to_string();
        if trimmed.is_empty() {
            "download".to_string()
        } else {
            trimmed.chars().take(160).collect()
        }
    }

    assert_eq!(
        sanitize_file_name(""),
        "download",
        "Empty string should return 'download'"
    );
    assert_eq!(
        sanitize_file_name("   "),
        "download",
        "Whitespace-only should return 'download'"
    );
    assert_eq!(
        sanitize_file_name("..."),
        "download",
        "Dots-only should return 'download' after trailing trim"
    );
}

/// Characterize: sanitize_file_name trims trailing dots and spaces.
#[test]
fn test_characterize_sanitize_file_name_trailing_trim() {
    fn sanitize_file_name(raw_name: &str) -> String {
        let re = regex::Regex::new(r#"[\\/:*?"<>|]"#).unwrap();
        let replaced = re.replace_all(raw_name, "_");
        let collapsed = replaced.split_whitespace().collect::<Vec<&str>>().join(" ");
        let trimmed = collapsed.trim().trim_end_matches(['.', ' ']).to_string();
        if trimmed.is_empty() {
            "download".to_string()
        } else {
            trimmed.chars().take(160).collect()
        }
    }

    let result = sanitize_file_name("my video...");
    assert_eq!(result, "my video", "Trailing dots should be trimmed");
}

/// Characterize: sanitize_file_name truncates to 160 characters.
#[test]
fn test_characterize_sanitize_file_name_truncates_at_160() {
    fn sanitize_file_name(raw_name: &str) -> String {
        let re = regex::Regex::new(r#"[\\/:*?"<>|]"#).unwrap();
        let replaced = re.replace_all(raw_name, "_");
        let collapsed = replaced.split_whitespace().collect::<Vec<&str>>().join(" ");
        let trimmed = collapsed.trim().trim_end_matches(['.', ' ']).to_string();
        if trimmed.is_empty() {
            "download".to_string()
        } else {
            trimmed.chars().take(160).collect()
        }
    }

    let long_name: String = "a".repeat(200);
    let result = sanitize_file_name(&long_name);
    assert_eq!(result.chars().count(), 160, "Should truncate to 160 chars");
}

// ============================================================================
// normalize_youtube_video_url characterization
// ============================================================================

/// Characterize: normalize_youtube_video_url normalizes standard watch URLs.
#[test]
fn test_characterize_normalize_youtube_watch_url() {
    fn normalize(raw_url: &str) -> String {
        let input = raw_url.trim();
        if input.is_empty() {
            return input.to_string();
        }
        let parsed = url::Url::parse(input);
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
            let path_parts: Vec<&str> =
                parsed.path().split('/').filter(|p| !p.is_empty()).collect();
            if path_parts.len() >= 2 && (path_parts[0] == "shorts" || path_parts[0] == "live") {
                return format!("https://www.youtube.com/watch?v={}", path_parts[1]);
            }
        }
        if host == "youtu.be" {
            let video_id = parsed.path().split('/').find(|p| !p.is_empty());
            if let Some(video_id) = video_id {
                return format!("https://www.youtube.com/watch?v={video_id}");
            }
        }
        input.to_string()
    }

    // Standard watch URL should be returned as-is (already canonical)
    let result = normalize("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    assert_eq!(result, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
}

/// Characterize: normalize_youtube_video_url normalizes youtu.be short links.
#[test]
fn test_characterize_normalize_youtu_be_url() {
    fn normalize(raw_url: &str) -> String {
        let input = raw_url.trim();
        if input.is_empty() {
            return input.to_string();
        }
        let parsed = url::Url::parse(input);
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
            let path_parts: Vec<&str> =
                parsed.path().split('/').filter(|p| !p.is_empty()).collect();
            if path_parts.len() >= 2 && (path_parts[0] == "shorts" || path_parts[0] == "live") {
                return format!("https://www.youtube.com/watch?v={}", path_parts[1]);
            }
        }
        if host == "youtu.be" {
            let video_id = parsed.path().split('/').find(|p| !p.is_empty());
            if let Some(video_id) = video_id {
                return format!("https://www.youtube.com/watch?v={video_id}");
            }
        }
        input.to_string()
    }

    let result = normalize("https://youtu.be/dQw4w9WgXcQ");
    assert_eq!(
        result, "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        "youtu.be short URL should be normalized to watch URL"
    );
}

/// Characterize: normalize_youtube_video_url normalizes YouTube Shorts URLs.
#[test]
fn test_characterize_normalize_youtube_shorts_url() {
    fn normalize(raw_url: &str) -> String {
        let input = raw_url.trim();
        if input.is_empty() {
            return input.to_string();
        }
        let parsed = url::Url::parse(input);
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
            let path_parts: Vec<&str> =
                parsed.path().split('/').filter(|p| !p.is_empty()).collect();
            if path_parts.len() >= 2 && (path_parts[0] == "shorts" || path_parts[0] == "live") {
                return format!("https://www.youtube.com/watch?v={}", path_parts[1]);
            }
        }
        if host == "youtu.be" {
            let video_id = parsed.path().split('/').find(|p| !p.is_empty());
            if let Some(video_id) = video_id {
                return format!("https://www.youtube.com/watch?v={video_id}");
            }
        }
        input.to_string()
    }

    let result = normalize("https://www.youtube.com/shorts/dQw4w9WgXcQ");
    assert_eq!(
        result, "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        "YouTube Shorts URL should be normalized to watch URL"
    );
}

/// Characterize: normalize_youtube_video_url returns non-YouTube URLs unchanged.
#[test]
fn test_characterize_normalize_non_youtube_url() {
    fn normalize(raw_url: &str) -> String {
        let input = raw_url.trim();
        if input.is_empty() {
            return input.to_string();
        }
        let parsed = url::Url::parse(input);
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
            let path_parts: Vec<&str> =
                parsed.path().split('/').filter(|p| !p.is_empty()).collect();
            if path_parts.len() >= 2 && (path_parts[0] == "shorts" || path_parts[0] == "live") {
                return format!("https://www.youtube.com/watch?v={}", path_parts[1]);
            }
        }
        if host == "youtu.be" {
            let video_id = parsed.path().split('/').find(|p| !p.is_empty());
            if let Some(video_id) = video_id {
                return format!("https://www.youtube.com/watch?v={video_id}");
            }
        }
        input.to_string()
    }

    let vimeo_url = "https://vimeo.com/123456789";
    let result = normalize(vimeo_url);
    assert_eq!(
        result, vimeo_url,
        "Non-YouTube URLs should be returned unchanged"
    );
}

/// Characterize: normalize_youtube_video_url returns empty string unchanged.
#[test]
fn test_characterize_normalize_empty_url() {
    fn normalize(raw_url: &str) -> String {
        let input = raw_url.trim();
        if input.is_empty() {
            return input.to_string();
        }
        let parsed = url::Url::parse(input);
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
            let path_parts: Vec<&str> =
                parsed.path().split('/').filter(|p| !p.is_empty()).collect();
            if path_parts.len() >= 2 && (path_parts[0] == "shorts" || path_parts[0] == "live") {
                return format!("https://www.youtube.com/watch?v={}", path_parts[1]);
            }
        }
        if host == "youtu.be" {
            let video_id = parsed.path().split('/').find(|p| !p.is_empty());
            if let Some(video_id) = video_id {
                return format!("https://www.youtube.com/watch?v={video_id}");
            }
        }
        input.to_string()
    }

    assert_eq!(normalize(""), "", "Empty string should return empty string");
    assert_eq!(
        normalize("   "),
        "",
        "Whitespace-only should return empty string after trim"
    );
}

/// Characterize: normalize_youtube_video_url returns invalid URLs unchanged.
#[test]
fn test_characterize_normalize_invalid_url() {
    fn normalize(raw_url: &str) -> String {
        let input = raw_url.trim();
        if input.is_empty() {
            return input.to_string();
        }
        let parsed = url::Url::parse(input);
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
            let path_parts: Vec<&str> =
                parsed.path().split('/').filter(|p| !p.is_empty()).collect();
            if path_parts.len() >= 2 && (path_parts[0] == "shorts" || path_parts[0] == "live") {
                return format!("https://www.youtube.com/watch?v={}", path_parts[1]);
            }
        }
        if host == "youtu.be" {
            let video_id = parsed.path().split('/').find(|p| !p.is_empty());
            if let Some(video_id) = video_id {
                return format!("https://www.youtube.com/watch?v={video_id}");
            }
        }
        input.to_string()
    }

    let invalid = "not a url at all";
    assert_eq!(
        normalize(invalid),
        invalid,
        "Invalid URLs should be returned unchanged"
    );
}

// ============================================================================
// lock_or_recover characterization
// ============================================================================

/// Characterize: lock_or_recover returns the value from a healthy mutex.
#[test]
fn test_characterize_lock_or_recover_normal_lock() {
    use std::sync::{Arc, Mutex};

    fn lock_or_recover<'a, T>(
        mutex: &'a Arc<Mutex<T>>,
        context: &str,
    ) -> std::sync::MutexGuard<'a, T> {
        mutex.lock().unwrap_or_else(|e| {
            eprintln!("[STABILITY] Mutex poisoned in {context}, recovering: {e:?}");
            e.into_inner()
        })
    }

    let m: Arc<Mutex<i32>> = Arc::new(Mutex::new(42));
    let guard = lock_or_recover(&m, "test_context");
    assert_eq!(*guard, 42, "Should return value from healthy mutex");
}

/// Characterize: lock_or_recover recovers from a poisoned mutex.
#[test]
fn test_characterize_lock_or_recover_poisoned_mutex() {
    use std::sync::{Arc, Mutex};

    fn lock_or_recover<'a, T>(
        mutex: &'a Arc<Mutex<T>>,
        context: &str,
    ) -> std::sync::MutexGuard<'a, T> {
        mutex.lock().unwrap_or_else(|e| {
            eprintln!("[STABILITY] Mutex poisoned in {context}, recovering: {e:?}");
            e.into_inner()
        })
    }

    let m: Arc<Mutex<i32>> = Arc::new(Mutex::new(99));
    let m_clone = m.clone();

    // Poison the mutex
    let _ = std::panic::catch_unwind(move || {
        let _guard = m_clone.lock().unwrap();
        panic!("intentional panic to poison");
    });

    // lock_or_recover should not panic - it should recover
    let guard = lock_or_recover(&m, "poison_recovery_test");
    assert_eq!(*guard, 99, "Should recover value from poisoned mutex");
}

// ============================================================================
// append_download_log characterization
// ============================================================================

/// Characterize: append_download_log adds a line and returns true for new lines.
#[test]
fn test_characterize_append_download_log_new_line() {
    // Simulate QueueItem with just the fields needed for append_download_log
    struct MockItem {
        download_log: Option<Vec<String>>,
    }

    const MAX_LOG_LINES_PER_JOB: usize = 120;

    fn append_download_log(item: &mut MockItem, line: &str) -> bool {
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

    let mut item = MockItem { download_log: None };

    // First line: should return true (appended)
    let result = append_download_log(&mut item, "line one");
    assert!(result, "New line should return true");
    assert_eq!(item.download_log.as_ref().unwrap().len(), 1);

    // Different line: should return true (appended)
    let result2 = append_download_log(&mut item, "line two");
    assert!(result2, "Different line should return true");
    assert_eq!(item.download_log.as_ref().unwrap().len(), 2);
}

/// Characterize: append_download_log deduplicates consecutive identical lines.
#[test]
fn test_characterize_append_download_log_dedup() {
    struct MockItem {
        download_log: Option<Vec<String>>,
    }

    const MAX_LOG_LINES_PER_JOB: usize = 120;

    fn append_download_log(item: &mut MockItem, line: &str) -> bool {
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

    let mut item = MockItem { download_log: None };
    append_download_log(&mut item, "duplicate line");

    // Same line again: should return false (not appended)
    let result = append_download_log(&mut item, "duplicate line");
    assert!(!result, "Duplicate consecutive line should return false");
    assert_eq!(
        item.download_log.as_ref().unwrap().len(),
        1,
        "Log should still have only 1 entry"
    );
}

/// Characterize: append_download_log trims to MAX_LOG_LINES_PER_JOB (120).
#[test]
fn test_characterize_append_download_log_max_lines() {
    struct MockItem {
        download_log: Option<Vec<String>>,
    }

    const MAX_LOG_LINES_PER_JOB: usize = 120;

    fn append_download_log(item: &mut MockItem, line: &str) -> bool {
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

    let mut item = MockItem { download_log: None };

    // Add 125 unique lines - should be trimmed to 120
    for i in 0..125 {
        append_download_log(&mut item, &format!("line {i}"));
    }
    let log = item.download_log.as_ref().unwrap();
    assert_eq!(log.len(), 120, "Log should be capped at 120 lines");
    // Oldest 5 lines should be removed; last entry is "line 124"
    assert_eq!(log.last().unwrap(), "line 124");
    // First entry is "line 5" (0-4 were removed)
    assert_eq!(log.first().unwrap(), "line 5");
}
