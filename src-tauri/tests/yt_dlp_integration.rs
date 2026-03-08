use std::env;
use std::process::Command;

const DEFAULT_TEST_URL: &str = "https://youtu.be/dQw4w9WgXcQ";

fn test_url() -> String {
    env::var("YT_DOWNLODER_TEST_URL").unwrap_or_else(|_| DEFAULT_TEST_URL.to_string())
}

fn find_yt_dlp() -> Option<String> {
    let candidates = ["yt-dlp", "/usr/local/bin/yt-dlp", "/opt/homebrew/bin/yt-dlp"];
    for candidate in candidates {
        if Command::new(candidate).arg("--version").output().is_ok() {
            return Some(candidate.to_string());
        }
    }
    None
}

#[test]
#[ignore]
fn test_yt_dlp_binary_available() {
    let binary = find_yt_dlp().expect("yt-dlp not found in PATH or common locations");
    let output = Command::new(&binary)
        .arg("--version")
        .output()
        .expect("failed to execute yt-dlp");

    assert!(output.status.success(), "yt-dlp --version returned non-zero exit code");
    let version = String::from_utf8_lossy(&output.stdout);
    assert!(!version.trim().is_empty(), "yt-dlp --version returned empty output");
    println!("yt-dlp version: {}", version.trim());
}

#[test]
#[ignore]
fn test_analyze_url_returns_valid_json() {
    let binary = find_yt_dlp().expect("yt-dlp not found");
    let url = test_url();

    let output = Command::new(&binary)
        .args(["--no-playlist", "-J", "--no-warnings", &url])
        .output()
        .expect("failed to execute yt-dlp");

    assert!(
        output.status.success(),
        "yt-dlp returned non-zero exit code: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("failed to parse yt-dlp JSON output");

    assert!(
        json.get("title").is_some(),
        "JSON missing 'title' field"
    );
    assert!(
        json.get("formats").is_some(),
        "JSON missing 'formats' field"
    );

    let title = json["title"].as_str().unwrap_or("");
    assert!(!title.is_empty(), "title field is empty");
    println!("Video title: {title}");
}

#[test]
#[ignore]
fn test_video_formats_available() {
    let binary = find_yt_dlp().expect("yt-dlp not found");
    let url = test_url();

    let output = Command::new(&binary)
        .args(["-F", &url])
        .output()
        .expect("failed to execute yt-dlp");

    assert!(
        output.status.success(),
        "yt-dlp -F returned non-zero exit code: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let format_list = String::from_utf8_lossy(&output.stdout);
    let has_video = format_list.contains("video") || format_list.contains("mp4");
    let has_audio = format_list.contains("audio") || format_list.contains("m4a");

    assert!(
        has_video,
        "No video formats found in yt-dlp -F output"
    );
    assert!(
        has_audio,
        "No audio formats found in yt-dlp -F output"
    );
    println!("Formats available:\n{format_list}");
}
