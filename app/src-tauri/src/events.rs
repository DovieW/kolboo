//! Centralized event name constants for Tauri events.
//!
//! All event strings that the backend emits should be defined here so they're
//! easy to find and refactor.

// ─────────────────────────────────────────────────────────────────────────────
// Pipeline events
// ─────────────────────────────────────────────────────────────────────────────

/// Emitted when the pipeline state changes (recording, transcribing, idle, etc.).
pub const EVENT_PIPELINE_STATE_CHANGED: &str = "pipeline-state-changed";

/// Emitted when an error occurs in the pipeline.
pub const EVENT_PIPELINE_ERROR: &str = "pipeline-error";

/// Emitted when a recording session starts (pipeline-specific).
pub const EVENT_PIPELINE_RECORDING_STARTED: &str = "pipeline-recording-started";

/// Emitted when transcription has started.
pub const EVENT_PIPELINE_TRANSCRIPTION_STARTED: &str = "pipeline-transcription-started";

/// Emitted when routing (preset selection) has started.
pub const EVENT_PIPELINE_ROUTING_STARTED: &str = "pipeline-routing-started";

/// Emitted when rewriting (LLM rewrite) has started.
pub const EVENT_PIPELINE_REWRITING_STARTED: &str = "pipeline-rewriting-started";

/// Emitted when the transcript is ready (after transcription completes).
pub const EVENT_PIPELINE_TRANSCRIPT_READY: &str = "pipeline-transcript-ready";

/// Emitted when the pipeline is reset to idle.
pub const EVENT_PIPELINE_RESET: &str = "pipeline-reset";

/// Emitted when the pipeline is cancelled by the user.
pub const EVENT_PIPELINE_CANCELLED: &str = "pipeline-cancelled";

// ─────────────────────────────────────────────────────────────────────────────
// Recording events
// ─────────────────────────────────────────────────────────────────────────────

/// Emitted when recording starts.
pub const EVENT_RECORDING_START: &str = "recording-start";

/// Emitted when recording stops.
pub const EVENT_RECORDING_STOP: &str = "recording-stop";

// ─────────────────────────────────────────────────────────────────────────────
// Streaming STT events
// ─────────────────────────────────────────────────────────────────────────────

/// Emitted when concurrent streaming STT produces a partial transcript
/// during an active recording. Payload is `{ text: string }`.
pub const EVENT_STT_PARTIAL_TRANSCRIPT: &str = "stt-partial-transcript";

// ─────────────────────────────────────────────────────────────────────────────
// Overlay events
// ─────────────────────────────────────────────────────────────────────────────

/// Emitted to send audio level updates to the overlay.
pub const EVENT_OVERLAY_AUDIO_LEVEL: &str = "overlay-audio-level";

/// Emitted to request that the overlay hide itself.
pub const EVENT_OVERLAY_HIDE_REQUESTED: &str = "overlay-hide-requested";

/// Emitted when the pipeline attempted to use active-window OCR but OCR context could not be produced.
///
/// The overlay should show a non-blocking message like "OCR context unavailable".
pub const EVENT_OVERLAY_OCR_CONTEXT_UNAVAILABLE: &str = "overlay-ocr-context-unavailable";

/// Emitted to report microphone test audio levels.
pub const EVENT_MIC_TEST_AUDIO_LEVEL: &str = "mic-test-audio-level";

// ─────────────────────────────────────────────────────────────────────────────
// History/UI events
// ─────────────────────────────────────────────────────────────────────────────

/// Emitted when the history list changes (new entry, edit, delete).
pub const EVENT_HISTORY_CHANGED: &str = "history-changed";

/// Emitted when stats data changes (usage/cost updates).
pub const EVENT_STATS_CHANGED: &str = "stats-changed";

/// Emitted when settings are updated and dependents should refresh.
pub const EVENT_SETTINGS_CHANGED: &str = "settings-changed";

/// Emitted when policy state changes and policy-aware surfaces should refresh.
pub const EVENT_POLICY_STATE_CHANGED: &str = "policy-state-changed";

/// Emitted when a transcript is copied to clipboard as a safe fallback.
pub const EVENT_TRANSCRIPT_COPIED_TO_CLIPBOARD: &str = "transcript-copied-to-clipboard";

/// Emitted when a second instance launch focuses the existing window.
pub const EVENT_SINGLE_INSTANCE_ACTIVATED: &str = "single-instance-activated";

/// Emitted for system-level events (e.g., sleep/wake, session lock).
pub const EVENT_SYSTEM_EVENT: &str = "system-event";

/// Emitted when the app detects a connection state change.
pub const EVENT_CONNECTION_STATE_CHANGED: &str = "connection-state-changed";

/// Emitted when the frontend should disconnect (e.g. connectivity failures).
pub const EVENT_REQUEST_DISCONNECT: &str = "request-disconnect";

// ─────────────────────────────────────────────────────────────────────────────
// Quick Ask events
// ─────────────────────────────────────────────────────────────────────────────

/// Emitted when a Quick Ask session starts.
pub const EVENT_QUICK_ASK_STARTED: &str = "quick-ask-started";

/// Emitted when a Quick Ask response is ready.
pub const EVENT_QUICK_ASK_ANSWER: &str = "quick-ask-answer";

/// Emitted to request that the Quick Ask overlay dismiss itself (animate out).
pub const EVENT_QUICK_ASK_DISMISS_REQUESTED: &str = "quick-ask-dismiss-requested";

// ─────────────────────────────────────────────────────────────────────────────
// Whisper/model events
// ─────────────────────────────────────────────────────────────────────────────

/// Emitted for local Whisper model load status changes.
pub const EVENT_LOCAL_WHISPER_MODEL_LOAD: &str = "local-whisper-model-load";

/// Emitted for Whisper model download progress updates.
pub const EVENT_WHISPER_MODEL_DOWNLOAD_PROGRESS: &str = "whisper-model-download-progress";
