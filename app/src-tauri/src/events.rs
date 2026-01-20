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

/// Emitted when transcription has started.
pub const EVENT_PIPELINE_TRANSCRIPTION_STARTED: &str = "pipeline-transcription-started";

/// Emitted when routing (preset selection) has started.
pub const EVENT_PIPELINE_ROUTING_STARTED: &str = "pipeline-routing-started";

/// Emitted when rewriting (LLM rewrite) has started.
pub const EVENT_PIPELINE_REWRITING_STARTED: &str = "pipeline-rewriting-started";

/// Emitted when the transcript is ready (after transcription completes).
pub const EVENT_PIPELINE_TRANSCRIPT_READY: &str = "pipeline-transcript-ready";

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
// Overlay events
// ─────────────────────────────────────────────────────────────────────────────

/// Emitted to send audio level updates to the overlay.
pub const EVENT_OVERLAY_AUDIO_LEVEL: &str = "overlay-audio-level";

/// Emitted to request that the overlay hide itself.
pub const EVENT_OVERLAY_HIDE_REQUESTED: &str = "overlay-hide-requested";

// ─────────────────────────────────────────────────────────────────────────────
// History/UI events
// ─────────────────────────────────────────────────────────────────────────────

/// Emitted when the history list changes (new entry, edit, delete).
pub const EVENT_HISTORY_CHANGED: &str = "history-changed";

/// Emitted for system-level events (e.g., sleep/wake, session lock).
pub const EVENT_SYSTEM_EVENT: &str = "system-event";
