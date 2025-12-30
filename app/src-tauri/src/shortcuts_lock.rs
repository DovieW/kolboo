// Shared lock for global shortcut (un)registration.
//
// Why this exists:
// - Multiple windows (main + overlay) can invoke commands concurrently.
// - The frontend sometimes calls unregister/register in quick succession.
// - Tauri's global shortcut manager can error with "HotKey already registered" when
//   two registrations overlap.
//
// Serializing access here makes shortcut registration idempotent and race-free.

#[cfg(desktop)]
use std::sync::OnceLock;

#[cfg(desktop)]
use tokio::sync::Mutex;

/// A process-wide mutex used to serialize all interactions with the global shortcut manager.
#[cfg(desktop)]
pub fn global_shortcut_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
