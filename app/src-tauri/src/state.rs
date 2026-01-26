use std::sync::atomic::{AtomicBool, AtomicU64};
#[cfg(desktop)]
use std::sync::Mutex;

#[cfg(desktop)]
use std::collections::VecDeque;

#[cfg(desktop)]
use std::sync::Arc;

#[cfg(desktop)]
use tokio_util::sync::CancellationToken;

#[cfg(desktop)]
use crate::windows_uia::types::WindowsTextContextSource;

#[derive(Default)]
pub struct AppState {
    /// Tracks if currently recording (for both toggle and hold modes)
    pub is_recording: AtomicBool,
    /// Monotonic token bumped every time we *show* the overlay window.
    ///
    /// Used to guard delayed-hide fallbacks against races where a previous session schedules
    /// a hide, but a new recording/error shows the overlay before that timer fires.
    pub overlay_visibility_epoch: AtomicU64,

    /// Monotonic token bumped every time we show/refresh the overlay hover window.
    /// Used to cancel delayed-hide timers when the pointer moves between windows.
    pub overlay_hover_epoch: AtomicU64,
    /// Tracks whether we toggled MediaPlayPause when recording started.
    /// Used to restore playback when recording ends.
    pub play_pause_toggled: AtomicBool,
    /// Tracks if PTT key is currently held down (for hold-to-record mode)
    pub ptt_key_held: AtomicBool,
    /// Tracks if paste-last key is currently held down
    pub paste_key_held: AtomicBool,
    /// Tracks if retry key is currently held down
    pub retry_key_held: AtomicBool,
    /// Tracks if quick ask key is currently held down
    pub quick_ask_key_held: AtomicBool,
    /// Tracks if quick ask toggle key is currently held down (for debouncing - action happens on release)
    pub quick_ask_toggle_key_held: AtomicBool,
    /// Tracks whether the current recording session was started as a Quick Ask.
    ///
    /// This is used to branch the stop-recording success path into an LLM answer
    /// overlay (instead of output/paste).
    pub quick_ask_session_active: AtomicBool,
    /// Tracks if toggle key is currently held down (for debouncing - action happens on release)
    pub toggle_key_held: AtomicBool,

    /// The program prompt profile id resolved at recording start (before overlays can steal focus).
    ///
    /// Used to apply per-profile behavior consistently during stop/transcribe.
    #[cfg(desktop)]
    pub recording_session_profile_id: Mutex<Option<String>>,

    /// Monotonic id for Quick Replace selection probes.
    ///
    /// Each transcription session can kick off at most one probe.
    pub quick_replace_probe_epoch: AtomicU64,

    /// Monotonic id for Quick Ask selection probes.
    ///
    /// Each Quick Ask session can kick off at most one probe.
    pub quick_ask_probe_epoch: AtomicU64,

    /// Latest Quick Replace selection probe result (ephemeral; never persisted).
    #[cfg(desktop)]
    pub quick_replace_probe: Mutex<QuickReplaceProbe>,

    /// Latest Quick Ask selection probe result (ephemeral; never persisted).
    #[cfg(desktop)]
    pub quick_ask_probe: Mutex<QuickAskProbe>,

    /// Ephemeral Windows text target snapshot captured near recording stop.
    #[cfg(desktop)]
    pub windows_text_target_snapshot:
        Mutex<Option<crate::windows_uia::types::WindowsTextTargetSnapshot>>,
}

/// A single Quick Ask conversation turn (question + answer).
#[derive(Debug, Clone)]
pub struct QuickAskConversationTurn {
    pub question: String,
    pub answer: String,
}

/// Ephemeral Quick Ask conversation history (in-memory only).
///
/// This is managed as its own Tauri state so we can safely access it from async
/// contexts without borrowing issues.
#[derive(Clone)]
pub struct QuickAskConversationMemory {
    #[cfg(desktop)]
    inner: Arc<Mutex<VecDeque<QuickAskConversationTurn>>>,
}

impl Default for QuickAskConversationMemory {
    fn default() -> Self {
        Self {
            #[cfg(desktop)]
            inner: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
}

impl QuickAskConversationMemory {
    /// Maximum number of turns retained in memory.
    ///
    /// This is independent of the UI setting (which controls how many are *sent*).
    pub const MAX_TURNS: usize = 50;

    /// Return up to the last `n` turns (oldest -> newest).
    pub fn snapshot_last(&self, n: usize) -> Vec<QuickAskConversationTurn> {
        #[cfg(desktop)]
        {
            let n = n.clamp(1, 20);
            if let Ok(history) = self.inner.lock() {
                let mut turns: Vec<QuickAskConversationTurn> =
                    history.iter().rev().take(n).cloned().collect();
                turns.reverse();
                return turns;
            }
        }

        Vec::new()
    }

    /// Push a successful Q/A turn into memory (best-effort).
    pub fn push_turn(&self, question: String, answer: String) {
        #[cfg(desktop)]
        {
            if let Ok(mut history) = self.inner.lock() {
                history.push_back(QuickAskConversationTurn { question, answer });
                while history.len() > Self::MAX_TURNS {
                    history.pop_front();
                }
            }
        }
    }
}

/// Ephemeral selection probe state used by the Quick Replace feature.
#[cfg(desktop)]
#[derive(Debug, Clone)]
pub struct QuickReplaceProbe {
    pub epoch: u64,
    pub ready: bool,
    pub selection_text: Option<String>,
    pub surrounding_text: Option<String>,
    pub source: WindowsTextContextSource,
}

#[cfg(desktop)]
impl Default for QuickReplaceProbe {
    fn default() -> Self {
        Self {
            epoch: 0,
            ready: true,
            selection_text: None,
            surrounding_text: None,
            source: WindowsTextContextSource::None,
        }
    }
}

/// Ephemeral selection probe state used by Quick Ask to capture highlighted context.
#[cfg(desktop)]
#[derive(Debug, Clone)]
pub struct QuickAskProbe {
    pub epoch: u64,
    pub ready: bool,
    pub selection_text: Option<String>,
    pub surrounding_text: Option<String>,
    pub source: WindowsTextContextSource,
}

#[cfg(desktop)]
impl Default for QuickAskProbe {
    fn default() -> Self {
        Self {
            epoch: 0,
            ready: true,
            selection_text: None,
            surrounding_text: None,
            source: WindowsTextContextSource::None,
        }
    }
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
