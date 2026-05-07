//! Shared request/recording retention helpers.
//!
//! Command flows and hotkey flows both apply the same time-based transcription retention policy.
//! Keeping the parsing + pruning logic here avoids re-spreading settings-store reads across
//! orchestration modules.

use chrono::{Duration as ChronoDuration, Utc};
use tauri::{AppHandle, Emitter, Manager};

use crate::events;
use crate::history::HistoryStorage;
use crate::recordings::RecordingStore;
use crate::settings::store::get_fresh_settings_store;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TranscriptionRetentionUnit {
    Days,
    Hours,
}

fn parse_transcription_retention_unit(raw: Option<&str>) -> Option<TranscriptionRetentionUnit> {
    raw.map(|value| match value {
        "hours" => TranscriptionRetentionUnit::Hours,
        // Keep legacy behavior: any explicit non-"hours" value is treated as days.
        _ => TranscriptionRetentionUnit::Days,
    })
}

fn parse_transcription_retention_value(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_u64().map(|raw| raw as f64))
        .or_else(|| value.as_str().and_then(|raw| raw.parse::<f64>().ok()))
}

fn duration_from_new_retention_keys(
    unit: TranscriptionRetentionUnit,
    value: f64,
) -> Option<ChronoDuration> {
    // `0` (or negative / NaN) means "keep forever".
    if value.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        return None;
    }

    match unit {
        TranscriptionRetentionUnit::Days => {
            // Defensive cap: 0..36500 days (~100 years).
            let days = value.round().clamp(0.0, 36_500.0) as i64;
            if days <= 0 {
                None
            } else {
                Some(ChronoDuration::days(days))
            }
        }
        TranscriptionRetentionUnit::Hours => {
            // Allow fractional hours (for example 0.5 hours).
            let hours = value.clamp(0.0, 36_500.0 * 24.0);
            if hours.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
                None
            } else {
                let millis = (hours * 3_600_000.0).round() as i64;
                Some(ChronoDuration::milliseconds(millis))
            }
        }
    }
}

fn transcription_retention_duration_from_parts(
    unit: Option<TranscriptionRetentionUnit>,
    value: Option<f64>,
    legacy_days: u64,
) -> Option<ChronoDuration> {
    if let (Some(unit), Some(value)) = (unit, value) {
        return duration_from_new_retention_keys(unit, value);
    }

    if legacy_days == 0 {
        None
    } else {
        Some(ChronoDuration::days(legacy_days as i64))
    }
}

fn get_transcription_retention_duration(app: &AppHandle) -> Option<ChronoDuration> {
    #[cfg(desktop)]
    {
        let store = get_fresh_settings_store(app);

        let unit = store
            .as_ref()
            .and_then(|settings| settings.get("transcription_retention_unit"))
            .and_then(|value| parse_transcription_retention_unit(value.as_str()));

        let value = store
            .as_ref()
            .and_then(|settings| settings.get("transcription_retention_value"))
            .and_then(|value| parse_transcription_retention_value(&value));

        let legacy_days = store
            .as_ref()
            .and_then(|settings| settings.get("transcription_retention_days"))
            .and_then(|value| {
                value
                    .as_u64()
                    .or_else(|| value.as_f64().map(|raw| raw as u64))
            })
            .unwrap_or(0);

        transcription_retention_duration_from_parts(unit, value, legacy_days)
    }

    #[cfg(not(desktop))]
    {
        let _ = app;
        None
    }
}

fn get_transcription_retention_delete_recordings(app: &AppHandle) -> bool {
    #[cfg(desktop)]
    {
        get_fresh_settings_store(app)
            .and_then(|store| store.get("transcription_retention_delete_recordings"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    }

    #[cfg(not(desktop))]
    {
        let _ = app;
        false
    }
}

/// Apply time-based transcription retention.
///
/// This is intentionally best-effort: failed pruning should never block the user from finishing a
/// request. Recording deletion mirrors the matching history deletions when enabled.
pub(crate) fn apply_transcription_retention(app: &AppHandle) {
    let Some(retention) = get_transcription_retention_duration(app) else {
        return;
    };

    let cutoff = Utc::now() - retention;
    let delete_recordings = get_transcription_retention_delete_recordings(app);

    let Some(history) = app.try_state::<HistoryStorage>() else {
        return;
    };

    let removed = match history.prune_older_than(cutoff) {
        Ok(ids) => ids,
        Err(error) => {
            log::warn!("Failed to prune history by time retention: {}", error);
            return;
        }
    };

    if removed.is_empty() {
        return;
    }

    if delete_recordings {
        if let Some(store) = app.try_state::<RecordingStore>() {
            for id in &removed {
                // Best-effort: missing files or delete failures should not prevent history prune.
                let _ = store.delete_wav_if_exists(id);
            }
        }
    }

    let _ = app.emit(events::EVENT_HISTORY_CHANGED, ());
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_retention_value_from_number_or_string() {
        assert_eq!(parse_transcription_retention_value(&json!(12)), Some(12.0));
        assert_eq!(parse_transcription_retention_value(&json!(0.5)), Some(0.5));
        assert_eq!(
            parse_transcription_retention_value(&json!("1.25")),
            Some(1.25)
        );
        assert_eq!(parse_transcription_retention_value(&json!("nope")), None);
    }

    #[test]
    fn supports_fractional_hour_retention() {
        assert_eq!(
            duration_from_new_retention_keys(TranscriptionRetentionUnit::Hours, 0.5),
            Some(ChronoDuration::minutes(30))
        );
    }

    #[test]
    fn falls_back_to_legacy_days_when_new_keys_are_missing() {
        assert_eq!(
            transcription_retention_duration_from_parts(None, None, 7),
            Some(ChronoDuration::days(7))
        );
    }

    #[test]
    fn zero_or_negative_retention_means_keep_forever() {
        assert_eq!(
            duration_from_new_retention_keys(TranscriptionRetentionUnit::Days, 0.0),
            None
        );
        assert_eq!(
            duration_from_new_retention_keys(TranscriptionRetentionUnit::Hours, -2.0),
            None
        );
    }
}
