//! Content-free activity summaries from retained, successful history entries.
use super::{HistoryEntry, HistoryStatus, HistoryStorage};
use chrono::{DateTime, Duration, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActivityTotals {
    pub recordings: u64,
    pub words: u64,
    pub audio_seconds: f64,
    pub timed_recordings: u64,
    pub meetings: u64,
}

impl ActivityTotals {
    fn add(&mut self, entry: &HistoryEntry) {
        self.recordings += 1;
        self.words += entry
            .original_stt_text
            .as_deref()
            .unwrap_or(&entry.text)
            .split_whitespace()
            .count() as u64;
        if let Some(seconds) = entry
            .duration_seconds
            .filter(|value| value.is_finite() && *value > 0.0)
        {
            self.audio_seconds += seconds;
            self.timed_recordings += 1;
        }
        if entry.recording_mode == Some(crate::RecordingMode::Meeting) {
            self.meetings += 1;
        }
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ActivityDay {
    pub date: String,
    pub totals: ActivityTotals,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ActivityModel {
    pub provider: String,
    pub model: String,
    pub totals: ActivityTotals,
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct HistoryActivity {
    pub totals: ActivityTotals,
    pub days: Vec<ActivityDay>,
    pub models: Vec<ActivityModel>,
}

fn summarize(entries: &[HistoryEntry], timeframe: &str, now: DateTime<Utc>) -> HistoryActivity {
    let cutoff = match timeframe {
        "24h" => Some(now - Duration::hours(24)),
        "7d" => Some(now - Duration::days(7)),
        "90d" => Some(now - Duration::days(90)),
        "all" => None,
        _ => Some(now - Duration::days(30)),
    };
    // A rerun is another result for the same recording. Keep its first successful
    // result and original date, even when a later rerun falls inside the range.
    let mut recordings: HashMap<&str, &HistoryEntry> = HashMap::new();
    for entry in entries
        .iter()
        .filter(|entry| entry.status == HistoryStatus::Success)
    {
        let key = entry
            .recording_request_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .unwrap_or(&entry.id);
        let earliest = recordings.entry(key).or_insert(entry);
        if entry.timestamp < earliest.timestamp {
            *earliest = entry;
        }
    }
    let mut result = HistoryActivity::default();
    let mut days = BTreeMap::<String, ActivityTotals>::new();
    let mut models = BTreeMap::<(String, String), ActivityTotals>::new();
    for entry in recordings.values().filter(|entry| {
        entry.timestamp <= now && cutoff.is_none_or(|start| entry.timestamp >= start)
    }) {
        result.totals.add(entry);
        days.entry(entry.timestamp.format("%Y-%m-%d").to_string())
            .or_default()
            .add(entry);
        models
            .entry((
                entry
                    .stt_provider
                    .clone()
                    .unwrap_or_else(|| "Unknown".into()),
                entry
                    .stt_model
                    .clone()
                    .unwrap_or_else(|| "Unknown model".into()),
            ))
            .or_default()
            .add(entry);
    }
    result.days = days
        .into_iter()
        .map(|(date, totals)| ActivityDay { date, totals })
        .collect();
    result.models = models
        .into_iter()
        .map(|((provider, model), totals)| ActivityModel {
            provider,
            model,
            totals,
        })
        .collect();
    result
        .models
        .sort_by(|left, right| right.totals.recordings.cmp(&left.totals.recordings));
    result
}

impl HistoryStorage {
    pub fn activity(&self, timeframe: &str) -> Result<HistoryActivity, String> {
        let data = self.data.read().map_err(|_| "Could not read activity")?;
        Ok(summarize(&data.entries, timeframe, Utc::now()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn activity_command_returns_only_aggregates_and_surfaces_read_errors() {
        let directory = tempfile::tempdir().unwrap();
        let storage = HistoryStorage::new(directory.path().to_path_buf());
        storage.data.write().unwrap().entries.push(entry(
            "example",
            Utc::now(),
            "private sample words",
        ));
        use tauri::Manager;
        let app = tauri::test::mock_builder()
            .manage(storage)
            .invoke_handler(tauri::generate_handler![
                crate::commands::history::get_history_activity
            ])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let result = crate::commands::history::get_history_activity("all".into(), app.state())
            .await
            .unwrap();
        assert_eq!(result.totals.words, 3);
        assert_eq!(result.totals.recordings, 1);
        assert!(!serde_json::to_string(&result).unwrap().contains("private"));
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let request = |body| tauri::webview::InvokeRequest {
            cmd: "get_history_activity".into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        };
        let response = tauri::test::get_ipc_response(
            &webview,
            request(serde_json::json!({"timeframe":"all"})),
        )
        .unwrap()
        .deserialize::<HistoryActivity>()
        .unwrap();
        assert_eq!(response.totals.words, 3);
        let invalid =
            tauri::test::get_ipc_response(&webview, request(serde_json::json!({}))).unwrap_err();
        assert!(invalid.as_str().unwrap().contains("timeframe"));
        let history = app.state::<HistoryStorage>();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = history.data.write().unwrap();
            panic!("poison the lock");
        }));
        assert!(
            crate::commands::history::get_history_activity("all".into(), app.state())
                .await
                .is_err()
        );
        let failed = tauri::test::get_ipc_response(
            &webview,
            request(serde_json::json!({"timeframe":"all"})),
        )
        .unwrap_err();
        assert!(failed.to_string().contains("Could not read activity"));
        assert!(!failed.to_string().contains("private sample words"));
    }

    fn entry(id: &str, timestamp: DateTime<Utc>, text: &str) -> HistoryEntry {
        let mut value = HistoryEntry::new(text.into());
        value.id = id.into();
        value.timestamp = timestamp;
        value
    }

    #[test]
    fn activity_counts_recordings_once_and_does_not_shift_reruns_into_new_periods() {
        let now = DateTime::parse_from_rfc3339("2026-09-23T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut original = entry("original", now - Duration::days(40), "one two");
        original.recording_request_id = Some("audio".into());
        let mut rerun = original.clone();
        rerun.id = "rerun".into();
        rerun.timestamp = now;
        let mut meeting = entry("meeting", now - Duration::hours(1), "rewritten");
        meeting.original_stt_text = Some("one two three four".into());
        meeting.duration_seconds = Some(3600.0);
        meeting.recording_mode = Some(crate::RecordingMode::Meeting);
        meeting.stt_provider = Some("openai".into());
        meeting.stt_model = Some("test-model".into());
        let mut failed = entry("failed", now, "error details");
        failed.status = HistoryStatus::Error;
        let mut pending = failed.clone();
        pending.status = HistoryStatus::InProgress;
        let future = entry("future", now + Duration::hours(1), "future");
        let values = vec![rerun, original, meeting, failed, pending, future];
        let recent = summarize(&values, "30d", now);
        assert_eq!(recent.totals.recordings, 1);
        assert_eq!(recent.totals.words, 4);
        assert_eq!(recent.totals.meetings, 1);
        assert_eq!(recent.totals.audio_seconds, 3600.0);
        assert_eq!(recent.totals.timed_recordings, 1);
        assert_eq!(recent.days[0].date, "2026-09-23");
        assert_eq!(recent.models[0].provider, "openai");
        assert_eq!(recent.models[0].model, "test-model");
        for timeframe in ["all", "90d"] {
            let all = summarize(&values, timeframe, now);
            assert_eq!(all.totals.recordings, 2);
            assert_eq!(all.totals.words, 6);
        }
        for timeframe in ["24h", "7d", "invalid"] {
            assert_eq!(summarize(&values, timeframe, now).totals.recordings, 1);
        }
    }

    #[test]
    fn empty_history_and_unknown_durations_do_not_invent_usage() {
        let now = Utc::now();
        assert_eq!(summarize(&[], "all", now).totals.recordings, 0);
        let mut legacy = entry("legacy", now, "  hello\nworld ");
        legacy.duration_seconds = Some(f64::NAN);
        legacy.recording_request_id = Some(String::new());
        let result = summarize(&[legacy], "all", now);
        assert_eq!(result.totals.words, 2);
        assert_eq!(result.totals.timed_recordings, 0);
        assert_eq!(result.totals.audio_seconds, 0.0);
        assert_eq!(result.models[0].provider, "Unknown");
    }
}
