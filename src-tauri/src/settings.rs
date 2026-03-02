use crate::file_ops::{normalize_download_dir, queue_file_path, settings_file_path, write_atomic};
use crate::types::CommandResult;
use dirs::download_dir;
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

// ============================================================================
// Domain types
// ============================================================================

/// Active application settings used at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub download_dir: String,
    pub max_retries: i32,
    pub language: String,
    pub max_concurrent_downloads: i32,
}

/// Partially-populated settings loaded from the persisted JSON file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedSettings {
    pub download_dir: Option<String>,
    pub max_retries: Option<i32>,
    pub language: Option<String>,
    pub max_concurrent_downloads: Option<i32>,
}

// ============================================================================
// AppState definition (used throughout the codebase)
// ============================================================================

/// Application runtime state: queue, settings, and worker count.
// @MX:ANCHOR: [AUTO] Central mutable state shared between worker threads and Tauri commands.
// @MX:REASON: [AUTO] High fan_in: all queue commands, settings commands, and the worker thread share this state.
#[derive(Debug, Clone)]
pub struct AppState {
    pub queue: Vec<crate::queue::QueueItem>,
    pub settings: AppSettings,
    pub active_worker_count: usize,
}

/// Thread-safe wrapper for AppState.
#[derive(Clone)]
pub struct SharedState(pub Arc<Mutex<AppState>>);

// ============================================================================
// Default settings
// ============================================================================

/// Creates the default AppSettings: uses the system download directory.
pub fn default_settings() -> AppSettings {
    AppSettings {
        download_dir: download_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .to_string_lossy()
            .to_string(),
        max_retries: 3,
        language: "ko".to_string(),
        max_concurrent_downloads: 2,
    }
}

// ============================================================================
// Persistence helpers
// ============================================================================

/// Persists settings to disk atomically, writing a backup after success.
// @MX:ANCHOR: [AUTO] Central settings persistence point — all configuration changes flow through here.
// @MX:REASON: [AUTO] High fan_in: set_settings command and initialization code call this to persist user preferences.
pub fn persist_settings(app: &AppHandle, settings: &AppSettings) {
    let path = settings_file_path(app);
    if let Ok(serialized) = serde_json::to_string_pretty(settings) {
        if write_atomic(&path, &serialized).is_ok() {
            let bak_path = PathBuf::from(format!("{}.bak", path.display()));
            let _ = write_atomic(&bak_path, &serialized);
        }
    }
}

/// Applies a successfully-parsed PersistedSettings into state.
pub fn apply_persisted_settings(state: &mut AppState, parsed: PersistedSettings) {
    if let Some(download_dir) = parsed.download_dir {
        state.settings.download_dir = normalize_download_dir(&download_dir);
    }
    if let Some(max_retries) = parsed.max_retries {
        state.settings.max_retries = max_retries.clamp(0, 10);
    }
    if let Some(language) = parsed.language {
        state.settings.language = language;
    }
    if let Some(max_concurrent) = parsed.max_concurrent_downloads {
        state.settings.max_concurrent_downloads = max_concurrent.clamp(1, 3);
    }
}

// ============================================================================
// Recovery loaders
// ============================================================================

/// Generic file-based loader with backup recovery.
///
/// Attempts to read `primary`, then `backup` on parse failure.
/// Returns `T::default()` if both fail.
fn load_json_with_recovery<T, F>(
    app: &AppHandle,
    primary: &Path,
    backup: &Path,
    mut apply_fn: F,
    corruption_recovered_event: &str,
    corruption_unrecoverable_event: &str,
) where
    T: serde::de::DeserializeOwned,
    F: FnMut(&AppHandle, T, bool),
{
    match fs::read_to_string(primary) {
        Err(_) => {
            // File missing: use defaults, no events emitted
        }
        Ok(content) => match serde_json::from_str::<T>(&content) {
            Ok(parsed) => {
                apply_fn(app, parsed, false);
            }
            Err(parse_err) => match fs::read_to_string(backup) {
                Ok(bak_content) => match serde_json::from_str::<T>(&bak_content) {
                    Ok(bak_parsed) => {
                        apply_fn(app, bak_parsed, true);
                        let _ = app.emit(
                            corruption_recovered_event,
                            serde_json::json!({
                                "message": "Restored from backup"
                            }),
                        );
                    }
                    Err(_) => {
                        let _ = app.emit(
                            corruption_unrecoverable_event,
                            serde_json::json!({
                                "error": parse_err.to_string()
                            }),
                        );
                    }
                },
                Err(_) => {
                    let _ = app.emit(
                        corruption_unrecoverable_event,
                        serde_json::json!({
                            "error": parse_err.to_string()
                        }),
                    );
                }
            },
        },
    }
}

/// Loads settings with backup recovery per SPEC-STABILITY-002 REQ-003.
// @MX:NOTE: [AUTO] Adds backup recovery and corruption events per SPEC-STABILITY-002.
pub fn load_settings_with_recovery(app: &AppHandle, state: &mut AppState) {
    let path = settings_file_path(app);
    let bak_path = PathBuf::from(format!("{}.bak", path.display()));

    load_json_with_recovery::<PersistedSettings, _>(
        app,
        &path,
        &bak_path,
        |_app, parsed, restored| {
            apply_persisted_settings(state, parsed);
            if restored {
                // Primary already rewritten inside apply; write backup of restored settings
                if let Ok(serialized) = serde_json::to_string_pretty(&state.settings) {
                    let _ = write_atomic(&path, &serialized);
                }
            } else if let Ok(serialized) = serde_json::to_string_pretty(&state.settings) {
                // Valid primary: write backup
                let _ = write_atomic(&bak_path, &serialized);
            }
        },
        "settings-corruption-recovered",
        "settings-corruption-unrecoverable",
    );
}

// ============================================================================
// Queue loading helpers (defined here because they depend on AppState)
// ============================================================================

/// Normalizes queue items after loading: resets downloading→queued, fills missing logs.
pub fn normalize_queue_items(items: &mut [crate::queue::QueueItem]) {
    for item in items.iter_mut() {
        if item.status == "downloading" {
            item.status = "queued".to_string();
        }
        if item.download_log.is_none() {
            item.download_log = Some(Vec::new());
        }
    }
}

/// Loads the queue with backup recovery per SPEC-STABILITY-002 REQ-001.
// @MX:NOTE: [AUTO] Adds backup recovery and corruption events per SPEC-STABILITY-002.
pub fn load_queue_with_recovery(app: &AppHandle, state: &mut AppState) {
    let path = queue_file_path(app);
    let bak_path = PathBuf::from(format!("{}.bak", path.display()));

    load_json_with_recovery::<Vec<crate::queue::QueueItem>, _>(
        app,
        &path,
        &bak_path,
        |_app, mut parsed, restored| {
            normalize_queue_items(&mut parsed);
            state.queue = parsed;
            if restored {
                if let Ok(serialized) = serde_json::to_string_pretty(&state.queue) {
                    let _ = write_atomic(&path, &serialized);
                }
            } else if let Ok(serialized) = serde_json::to_string_pretty(&state.queue) {
                let _ = write_atomic(&bak_path, &serialized);
            }
        },
        "queue-corruption-recovered",
        "queue-corruption-unrecoverable",
    );
}

// ============================================================================
// Tauri commands
// ============================================================================

/// Returns the current application settings.
#[tauri::command]
pub async fn get_settings(state: State<'_, SharedState>) -> CommandResult<AppSettings> {
    let state = state
        .0
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    Ok(state.settings.clone())
}

/// Validates, applies, and persists new application settings.
#[tauri::command]
pub async fn set_settings(
    app: AppHandle,
    state: State<'_, SharedState>,
    settings: AppSettings,
) -> CommandResult<()> {
    let mut state = state
        .0
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    state.settings = AppSettings {
        download_dir: normalize_download_dir(&settings.download_dir),
        max_retries: settings.max_retries.clamp(0, 10),
        language: settings.language,
        max_concurrent_downloads: settings.max_concurrent_downloads.clamp(1, 3),
    };
    persist_settings(&app, &state.settings);
    Ok(())
}

/// Opens a native folder picker and returns the selected directory path, or `None` if cancelled.
#[tauri::command]
pub async fn pick_download_dir() -> CommandResult<Option<String>> {
    let selected = FileDialog::new().pick_folder();
    Ok(selected.map(|path| path.to_string_lossy().to_string()))
}
