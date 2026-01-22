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

/// Emit `settings-changed` so other windows (and the UI) can refresh cached settings.
#[cfg(desktop)]
pub(crate) fn emit_settings_changed<T>(app: &AppHandle, payload: T)
where
    T: serde::Serialize + Clone,
{
    let _ = app.emit(events::EVENT_SETTINGS_CHANGED, payload);
}

/// Extract a filesystem-path basename for logs.
///
/// This avoids logging full paths (usernames, install locations) while keeping logs useful.
pub(crate) fn basename_for_log(path: &str) -> &str {
    let trimmed = path.trim().trim_matches('"');
    trimmed.rsplit(['\\', '/']).next().unwrap_or(trimmed)
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

/// Normalize an optional string from settings/user input.
///
/// Behavior:
/// - trims whitespace
/// - returns `None` if the trimmed value is empty
#[cfg_attr(not(desktop), allow(dead_code))]
pub(crate) fn normalize_optional_string(raw: Option<String>) -> Option<String> {
    raw.and_then(|s| {
        let t = s.trim().to_string();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    })
}

/// Normalize an optional base URL from settings/user input.
///
/// Behavior:
/// - trims whitespace
/// - trims trailing slashes
/// - returns `None` if the normalized value is empty
#[cfg_attr(not(desktop), allow(dead_code))]
pub(crate) fn normalize_optional_base_url(raw: Option<String>) -> Option<String> {
    raw.and_then(|s| {
        let t = s.trim().trim_end_matches('/').to_string();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_optional_string_trims_and_drops_empty() {
        assert_eq!(normalize_optional_string(None), None);
        assert_eq!(normalize_optional_string(Some("".to_string())), None);
        assert_eq!(normalize_optional_string(Some("   ".to_string())), None);
        assert_eq!(
            normalize_optional_string(Some("  hello  ".to_string())),
            Some("hello".to_string())
        );
    }

    #[test]
    fn normalize_optional_base_url_trims_and_strips_trailing_slashes() {
        assert_eq!(normalize_optional_base_url(None), None);
        assert_eq!(normalize_optional_base_url(Some("".to_string())), None);
        assert_eq!(
            normalize_optional_base_url(Some("  https://example.com/  ".to_string())),
            Some("https://example.com".to_string())
        );
        assert_eq!(
            normalize_optional_base_url(Some("https://example.com///".to_string())),
            Some("https://example.com".to_string())
        );
    }
}
