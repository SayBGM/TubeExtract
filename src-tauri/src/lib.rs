mod dependencies;
mod diagnostics;
mod download;
mod file_ops;
mod metadata;
mod queue;
mod settings;
mod state;
mod types;
mod utils;

// Re-export symbols used by integration tests (stability_tests.rs).
pub use crate::download::{classify_download_error, retry_delay_ms_for_strategy, RetryStrategy};

use crate::dependencies::{
    default_dependency_status, emit_dependency_status, start_dependency_bootstrap_if_needed,
    DependencyBootstrapStatus, DependencyRuntimeState, SharedDependencyState,
};
use crate::download::{RuntimeState, SharedRuntime};
use crate::file_ops::{remove_directory_safe, temp_downloads_root_dir};
use crate::queue::emit_queue_updated;
use crate::settings::{
    default_settings, load_queue_with_recovery, load_settings_with_recovery, SharedState,
};
use crate::types::CommandResult;
use std::sync::{Arc, Mutex};
use tauri::Manager;

// ============================================================================
// Dependency bootstrap status command
// ============================================================================

#[tauri::command]
async fn get_dependency_bootstrap_status(
    dependency: tauri::State<'_, SharedDependencyState>,
) -> CommandResult<DependencyBootstrapStatus> {
    let state = dependency
        .0
        .lock()
        .map_err(|_| "dependency state lock poisoned".to_string())?;
    Ok(state.status.clone())
}

// ============================================================================
// Application entry point
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .setup(|app| {
            remove_directory_safe(&temp_downloads_root_dir(app.handle()));

            let mut initial_state = crate::settings::AppState {
                queue: Vec::new(),
                settings: default_settings(),
                active_worker_count: 0,
            };
            load_settings_with_recovery(app.handle(), &mut initial_state);
            load_queue_with_recovery(app.handle(), &mut initial_state);
            crate::queue::scan_incomplete_markers(app.handle(), &mut initial_state);
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
                    emit_queue_updated(app.handle(), &locked);
                }
            }
            emit_dependency_status(
                app.handle(),
                &DependencyBootstrapStatus {
                    in_progress: true,
                    phase: "preparing".to_string(),
                    progress_percent: Some(5),
                    error_message: None,
                },
            );
            start_dependency_bootstrap_if_needed(app.handle().clone(), dependency_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            metadata::analyze_url,
            queue::check_duplicate,
            queue::enqueue_job,
            queue::pause_job,
            queue::resume_job,
            queue::cancel_job,
            queue::clear_terminal_jobs,
            queue::get_queue_snapshot,
            settings::get_settings,
            get_dependency_bootstrap_status,
            settings::pick_download_dir,
            settings::set_settings,
            diagnostics::run_diagnostics,
            diagnostics::check_update,
            diagnostics::get_storage_stats,
            queue::delete_file,
            diagnostics::open_folder,
            diagnostics::open_external_url,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                let app = window.app_handle().clone();
                if let Some(runtime) = app.try_state::<SharedRuntime>() {
                    if let Ok(mut rt) = runtime.0.lock() {
                        let txs: Vec<_> = rt.shutdown_txs.drain(..).collect();
                        for tx in txs {
                            let _ = tx.send(());
                        }
                        eprintln!(
                            "[STABILITY] Sent shutdown signal to {} worker thread(s)",
                            rt.worker_handles.len()
                        );
                    }
                }
            }
        });

    if let Err(e) = builder.run(tauri::generate_context!()) {
        eprintln!("[FATAL] Tauri initialization failed: {:?}", e);
        std::process::exit(1);
    }
}
