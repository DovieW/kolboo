use std::sync::atomic::AtomicBool;
#[cfg(desktop)]
use std::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    /// Tracks if currently recording (for both toggle and hold modes)
    pub is_recording: AtomicBool,
    /// Tracks whether we toggled MediaPlayPause when recording started.
    /// Used to restore playback when recording ends.
    pub play_pause_toggled: AtomicBool,
    /// Tracks if PTT key is currently held down (for hold-to-record mode)
    pub ptt_key_held: AtomicBool,
    /// Tracks if paste-last key is currently held down
    pub paste_key_held: AtomicBool,
    /// Tracks if toggle key is currently held down (for debouncing - action happens on release)
    pub toggle_key_held: AtomicBool,
}

/// Keeps the tray icon handle alive for the lifetime of the app.
///
/// On some platforms/builds, dropping the returned `TrayIcon` handle can effectively detach
/// tray callbacks even if the icon remains visible.
#[derive(Default)]
pub struct TrayKeepAlive {
    #[cfg(desktop)]
    pub tray: Mutex<Option<tauri::tray::TrayIcon<tauri::Wry>>>,
}

#[cfg(desktop)]
impl TrayKeepAlive {
    pub fn set(&self, tray: tauri::tray::TrayIcon<tauri::Wry>) {
        if let Ok(mut slot) = self.tray.lock() {
            *slot = Some(tray);
        }
    }
}
