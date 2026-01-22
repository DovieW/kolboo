use tauri::{AppHandle, Emitter};

use crate::event_payloads::SystemEvent;
use crate::events;

/// Helper to read a setting from the store with a default fallback.
#[cfg(desktop)]
pub(crate) fn get_setting_from_store<T: serde::de::DeserializeOwned>(
    app: &AppHandle,
    key: &str,
    default: T,
) -> T {
    use tauri_plugin_store::StoreExt;

    app.store("settings.json")
        .ok()
        .and_then(|store| store.get(key))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(default)
}

/// Emit a system event to the frontend for debugging.
#[cfg(desktop)]
pub(crate) fn emit_system_event(
    app: &AppHandle,
    event_type: &str,
    message: &str,
    details: Option<&str>,
) {
    let event = SystemEvent {
        timestamp: chrono::Utc::now().to_rfc3339(),
        event_type: event_type.to_string(),
        message: message.to_string(),
        details: details.map(|s| s.to_string()),
    };

    let _ = app.emit(events::EVENT_SYSTEM_EVENT, event);
}

/// Normalize transcript text for output.
///
/// We intentionally keep this conservative: the pipeline now performs a
/// quiet-audio gate before STT to avoid "silent audio" hallucinations.
pub(crate) fn sanitize_transcript(transcript: &str) -> Option<String> {
    let trimmed = transcript.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
