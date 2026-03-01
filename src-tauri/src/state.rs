use std::sync::{Arc, Mutex, MutexGuard};

/// Acquires a mutex lock, recovering from poisoning if necessary.
///
/// On poison, logs a warning to stderr and returns the inner value
/// so that callers can continue operating normally.
///
/// # Arguments
/// * `mutex` - The `Arc<Mutex<T>>` to lock
/// * `context` - A description used in the log message on recovery
// @MX:ANCHOR: [AUTO] Used in all mutex acquisition sites throughout lib.rs
// @MX:REASON: [AUTO] Central recovery point for SPEC-STABILITY-005 mutex poison handling
pub fn lock_or_recover<'a, T>(mutex: &'a Arc<Mutex<T>>, context: &str) -> MutexGuard<'a, T> {
    mutex.lock().unwrap_or_else(|e| {
        eprintln!("[STABILITY] Mutex poisoned in {context}, recovering: {e:?}");
        e.into_inner()
    })
}
