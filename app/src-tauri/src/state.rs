use std::sync::atomic::{AtomicBool, AtomicU64};
#[cfg(desktop)]
use std::sync::Mutex;

#[cfg(desktop)]
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct AppState {
    /// Tracks if currently recording (for both toggle and hold modes)
    pub is_recording: AtomicBool,
    /// Monotonic token bumped every time we *show* the overlay window.
    ///
    /// Used to guard delayed-hide fallbacks against races where a previous session schedules
    /// a hide, but a new recording/error shows the overlay before that timer fires.
    pub overlay_visibility_epoch: AtomicU64,
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

/// Backend-managed microphone test meter.
///
/// This is used by the Settings UI to display a realtime input level bar so users
/// can identify which microphone is which.
#[derive(Default)]
pub struct MicTestMeterState {
    #[cfg(desktop)]
    pub inner: Mutex<MicTestMeterInner>,
}

#[cfg(desktop)]
#[derive(Default)]
pub struct MicTestMeterInner {
    /// Cancellation token for the publisher loop.
    pub cancel: Option<CancellationToken>,
    /// If the mic test temporarily overrides the pipeline capture behavior,
    /// store the previous values so we can restore on stop.
    pub restore: Option<MicTestPipelineRestore>,

    /// Monotonic session id for mic test. Bumped each time we (re)start.
    pub session_id: u64,
}

#[cfg(desktop)]
#[derive(Debug, Clone)]
pub struct MicTestPipelineRestore {
    pub hot_mic_enabled: bool,
    pub hot_mic_pre_roll_ms: u32,
    pub mic_auto_recover_enabled: bool,
    pub input_device_name: Option<String>,
}

#[cfg(desktop)]
impl TrayKeepAlive {
    pub fn set(&self, tray: tauri::tray::TrayIcon<tauri::Wry>) {
        if let Ok(mut slot) = self.tray.lock() {
            *slot = Some(tray);
        }
    }
}
