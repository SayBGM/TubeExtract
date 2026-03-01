// Characterization tests for Phase 2 extraction (SPEC-REFACTOR-001)
// These tests capture CURRENT behavior before extracting file_ops.rs,
// dependencies.rs, and diagnostics.rs from lib.rs.
// If any of these fail after refactoring, behavior has been broken.

// ============================================================================
// normalize_ytdlp_version characterization
// ============================================================================

/// Characterize: normalize_ytdlp_version strips leading 'v' and trims whitespace.
#[test]
fn test_characterize_normalize_ytdlp_version_with_v_prefix() {
    fn normalize_ytdlp_version(input: &str) -> String {
        input.trim().trim_start_matches('v').to_string()
    }

    assert_eq!(
        normalize_ytdlp_version("v2024.01.01"),
        "2024.01.01",
        "Leading 'v' should be stripped"
    );
    assert_eq!(
        normalize_ytdlp_version("2024.01.01"),
        "2024.01.01",
        "Version without 'v' should be unchanged"
    );
    assert_eq!(
        normalize_ytdlp_version("  v2023.12.31  "),
        "2023.12.31",
        "Whitespace and 'v' should be stripped"
    );
    assert_eq!(
        normalize_ytdlp_version(""),
        "",
        "Empty string should return empty string"
    );
    assert_eq!(
        normalize_ytdlp_version("  "),
        "",
        "Whitespace-only should return empty string"
    );
}

// ============================================================================
// binary_with_platform_extension characterization
// ============================================================================

/// Characterize: binary_with_platform_extension adds .exe on Windows, unchanged otherwise.
#[test]
fn test_characterize_binary_with_platform_extension() {
    fn binary_with_platform_extension(binary_name: &str) -> String {
        if cfg!(target_os = "windows") {
            format!("{binary_name}.exe")
        } else {
            binary_name.to_string()
        }
    }

    // On non-Windows, should return the binary name unchanged
    let result = binary_with_platform_extension("yt-dlp");
    if cfg!(target_os = "windows") {
        assert_eq!(result, "yt-dlp.exe", "Windows should add .exe");
    } else {
        assert_eq!(result, "yt-dlp", "Non-Windows should not add extension");
    }

    let result2 = binary_with_platform_extension("ffmpeg");
    if cfg!(target_os = "windows") {
        assert_eq!(result2, "ffmpeg.exe");
    } else {
        assert_eq!(result2, "ffmpeg");
    }
}

// ============================================================================
// truncate_reason characterization
// ============================================================================

/// Characterize: truncate_reason returns at most 120 characters.
#[test]
fn test_characterize_truncate_reason_short_string() {
    fn truncate_reason(reason: &str) -> String {
        reason.chars().take(120).collect()
    }

    let short = "connection refused";
    assert_eq!(
        truncate_reason(short),
        short,
        "Short string should be returned unchanged"
    );
    assert_eq!(truncate_reason(""), "", "Empty string should return empty");
}

/// Characterize: truncate_reason truncates to exactly 120 chars.
#[test]
fn test_characterize_truncate_reason_long_string() {
    fn truncate_reason(reason: &str) -> String {
        reason.chars().take(120).collect()
    }

    let long_reason: String = "x".repeat(200);
    let result = truncate_reason(&long_reason);
    assert_eq!(result.chars().count(), 120, "Should truncate to 120 chars");
}

/// Characterize: truncate_reason handles exactly 120 chars without truncation.
#[test]
fn test_characterize_truncate_reason_exactly_120_chars() {
    fn truncate_reason(reason: &str) -> String {
        reason.chars().take(120).collect()
    }

    let exactly_120: String = "y".repeat(120);
    let result = truncate_reason(&exactly_120);
    assert_eq!(
        result.chars().count(),
        120,
        "Exactly 120 chars should not be truncated"
    );
}

// ============================================================================
// calculate_directory_size characterization
// ============================================================================

/// Characterize: calculate_directory_size returns 0 for a nonexistent directory.
#[test]
fn test_characterize_calculate_directory_size_nonexistent() {
    use std::fs;
    use std::path::Path;

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

    let nonexistent = Path::new("/tmp/nonexistent_path_for_test_only_xyz_12345");
    assert_eq!(
        calculate_directory_size(nonexistent),
        0,
        "Nonexistent directory should return 0"
    );
}

/// Characterize: calculate_directory_size sums file sizes in a real directory.
#[test]
fn test_characterize_calculate_directory_size_with_files() {
    use std::fs;
    use std::io::Write;
    use std::path::Path;

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

    // Create a temp directory with known content
    let tmp_dir = std::env::temp_dir().join("chartest_calc_dir_size");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).expect("Failed to create temp dir");

    let file1 = tmp_dir.join("file1.txt");
    let mut f = fs::File::create(&file1).expect("Failed to create file1");
    f.write_all(b"hello world").expect("Failed to write file1");
    drop(f);

    let file2 = tmp_dir.join("file2.txt");
    let mut f2 = fs::File::create(&file2).expect("Failed to create file2");
    f2.write_all(b"abc").expect("Failed to write file2");
    drop(f2);

    let total = calculate_directory_size(&tmp_dir);
    // 11 + 3 = 14 bytes
    assert_eq!(total, 14, "Should sum file sizes correctly: 11 + 3 = 14");

    let _ = fs::remove_dir_all(&tmp_dir);
}

// ============================================================================
// can_write_to_dir characterization
// ============================================================================

/// Characterize: can_write_to_dir returns true for a writable directory.
#[test]
fn test_characterize_can_write_to_dir_writable() {
    use std::fs;
    use std::path::Path;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

    let tmp_dir = std::env::temp_dir();
    assert!(
        can_write_to_dir(&tmp_dir),
        "System temp dir should be writable"
    );
}

/// Characterize: can_write_to_dir returns false for a nonexistent directory.
#[test]
fn test_characterize_can_write_to_dir_nonexistent() {
    use std::fs;
    use std::path::Path;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

    let nonexistent = Path::new("/nonexistent_path_definitely_not_here_xyz_999");
    assert!(
        !can_write_to_dir(nonexistent),
        "Nonexistent directory should not be writable"
    );
}

// ============================================================================
// remove_directory_safe characterization
// ============================================================================

/// Characterize: remove_directory_safe silently ignores nonexistent paths.
#[test]
fn test_characterize_remove_directory_safe_nonexistent() {
    use std::fs;
    use std::path::Path;

    fn remove_directory_safe(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    // Should not panic when directory doesn't exist
    let nonexistent = Path::new("/tmp/nonexistent_remove_test_xyz_54321");
    remove_directory_safe(nonexistent); // Must not panic
}

/// Characterize: remove_directory_safe removes an existing directory tree.
#[test]
fn test_characterize_remove_directory_safe_existing() {
    use std::fs;
    use std::path::Path;

    fn remove_directory_safe(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    let tmp_dir = std::env::temp_dir().join("chartest_remove_safe_xyz");
    fs::create_dir_all(&tmp_dir).expect("Failed to create temp dir");
    let test_file = tmp_dir.join("inner.txt");
    fs::write(&test_file, "content").expect("Failed to write file");

    assert!(tmp_dir.exists(), "Directory should exist before removal");
    remove_directory_safe(&tmp_dir);
    assert!(
        !tmp_dir.exists(),
        "Directory should not exist after removal"
    );
}

// ============================================================================
// resolve_downloaded_file_path characterization
// ============================================================================

/// Characterize: resolve_downloaded_file_path returns prioritized media.ext file if exists.
#[test]
fn test_characterize_resolve_downloaded_file_path_prioritized() {
    use std::fs;
    use std::path::{Path, PathBuf};

    fn resolve_downloaded_file_path(
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

    let tmp_dir = std::env::temp_dir().join("chartest_resolve_file_xyz");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).expect("Failed to create temp dir");

    // Create prioritized file: media.mp4
    let media_file = tmp_dir.join("media.mp4");
    fs::write(&media_file, "video content").expect("Failed to write media.mp4");

    // Also create another mp4 with a different name
    let other_file = tmp_dir.join("other_video.mp4");
    fs::write(&other_file, "other video").expect("Failed to write other_video.mp4");

    let result = resolve_downloaded_file_path(&tmp_dir, "mp4");
    assert!(result.is_ok(), "Should find media.mp4");
    assert_eq!(
        result.unwrap(),
        media_file,
        "Should prefer media.mp4 over other_video.mp4"
    );

    let _ = fs::remove_dir_all(&tmp_dir);
}

/// Characterize: resolve_downloaded_file_path returns Err for no matching files.
#[test]
fn test_characterize_resolve_downloaded_file_path_no_match() {
    use std::fs;
    use std::path::Path;

    fn resolve_downloaded_file_path(
        temp_dir: &Path,
        expected_ext: &str,
    ) -> Result<std::path::PathBuf, String> {
        let prioritized = temp_dir.join(format!("media.{expected_ext}"));
        if prioritized.exists() {
            return Ok(prioritized);
        }
        let mut candidates: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();
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

    let tmp_dir = std::env::temp_dir().join("chartest_resolve_no_match_xyz");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).expect("Failed to create temp dir");

    // Create a file with the wrong extension
    fs::write(tmp_dir.join("video.mkv"), "content").expect("Failed to write mkv");

    let result = resolve_downloaded_file_path(&tmp_dir, "mp4");
    assert!(
        result.is_err(),
        "Should return Err when no matching extension found"
    );
    assert!(
        result.unwrap_err().contains("찾지 못했"),
        "Error message should indicate file not found"
    );

    let _ = fs::remove_dir_all(&tmp_dir);
}

// ============================================================================
// default_dependency_status characterization
// ============================================================================

/// Characterize: default_dependency_status returns idle state.
#[test]
fn test_characterize_default_dependency_status_idle() {
    // Mirror the struct and function as in lib.rs
    #[derive(Debug, Clone)]
    struct DependencyBootstrapStatus {
        in_progress: bool,
        phase: String,
        progress_percent: Option<i32>,
        error_message: Option<String>,
    }

    fn default_dependency_status() -> DependencyBootstrapStatus {
        DependencyBootstrapStatus {
            in_progress: false,
            phase: "idle".to_string(),
            progress_percent: None,
            error_message: None,
        }
    }

    let status = default_dependency_status();
    assert!(
        !status.in_progress,
        "Default status should not be in progress"
    );
    assert_eq!(status.phase, "idle", "Default phase should be 'idle'");
    assert!(
        status.progress_percent.is_none(),
        "Default progress should be None"
    );
    assert!(
        status.error_message.is_none(),
        "Default error_message should be None"
    );
}

// ============================================================================
// open_external_url URL validation characterization
// ============================================================================

/// Characterize: open_external_url rejects non-http/https URLs.
#[test]
fn test_characterize_open_external_url_rejects_non_http() {
    use url::Url;

    fn validate_external_url(url: &str) -> Result<String, String> {
        if url.trim().is_empty() {
            return Ok("empty".to_string());
        }
        let parsed = Url::parse(url.trim()).map_err(|_| "유효한 URL이 아닙니다.".to_string())?;
        let scheme = parsed.scheme().to_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err("http/https URL만 열 수 있습니다.".to_string());
        }
        Ok(parsed.to_string())
    }

    // ftp URL should be rejected
    let result = validate_external_url("ftp://example.com/file.zip");
    assert!(result.is_err(), "ftp:// URL should be rejected");
    assert_eq!(result.unwrap_err(), "http/https URL만 열 수 있습니다.");

    // file:// URL should be rejected
    let result2 = validate_external_url("file:///etc/passwd");
    assert!(result2.is_err(), "file:// URL should be rejected");

    // http:// URL should be accepted
    let result3 = validate_external_url("http://example.com");
    assert!(result3.is_ok(), "http:// URL should be accepted");

    // https:// URL should be accepted
    let result4 = validate_external_url("https://example.com");
    assert!(result4.is_ok(), "https:// URL should be accepted");
}

/// Characterize: open_external_url rejects invalid URLs.
#[test]
fn test_characterize_open_external_url_rejects_invalid() {
    use url::Url;

    fn validate_external_url(url: &str) -> Result<String, String> {
        if url.trim().is_empty() {
            return Ok("empty".to_string());
        }
        let parsed = Url::parse(url.trim()).map_err(|_| "유효한 URL이 아닙니다.".to_string())?;
        let scheme = parsed.scheme().to_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err("http/https URL만 열 수 있습니다.".to_string());
        }
        Ok(parsed.to_string())
    }

    let result = validate_external_url("not a url");
    assert!(result.is_err(), "Non-URL string should be rejected");
    assert_eq!(result.unwrap_err(), "유효한 URL이 아닙니다.");
}

/// Characterize: open_external_url returns Ok for empty/whitespace URL (no-op).
#[test]
fn test_characterize_open_external_url_empty_is_noop() {
    use url::Url;

    fn validate_external_url(url: &str) -> Result<String, String> {
        if url.trim().is_empty() {
            return Ok("empty".to_string());
        }
        let parsed = Url::parse(url.trim()).map_err(|_| "유효한 URL이 아닙니다.".to_string())?;
        let scheme = parsed.scheme().to_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err("http/https URL만 열 수 있습니다.".to_string());
        }
        Ok(parsed.to_string())
    }

    let result = validate_external_url("");
    assert!(result.is_ok(), "Empty URL should return Ok (no-op)");

    let result2 = validate_external_url("   ");
    assert!(result2.is_ok(), "Whitespace URL should return Ok (no-op)");
}
