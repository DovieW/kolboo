use crate::history::{HistoryStatus, HistoryStorage, RequestHistoryUpdate, RequestModelInfo};
use chrono::{Duration as ChronoDuration, Utc};
use std::fs;
use std::path::PathBuf;

fn make_temp_dir(label: &str) -> PathBuf {
    let id = uuid::Uuid::new_v4().to_string();
    let dir = std::env::temp_dir().join(format!("kolboo-test-{label}-{id}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn test_history_add_and_complete() {
    let dir = make_temp_dir("history");
    let history = HistoryStorage::new(dir.clone());

    let _ = history.add_entry("first".to_string(), Some(1)).unwrap();
    let _ = history.add_entry("second".to_string(), Some(1)).unwrap();
    let all = history.get_all(None).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].text, "second");

    let req_id = "req-123".to_string();
    let _ = history
        .add_request_entry(req_id.clone(), RequestModelInfo::default(), Some(10))
        .unwrap();
    let entry = history.get_by_id(&req_id).unwrap().expect("entry");
    assert_eq!(entry.status, HistoryStatus::InProgress);

    history
        .complete_request_success(&req_id, "done".to_string())
        .unwrap();
    let entry = history.get_by_id(&req_id).unwrap().expect("entry");
    assert_eq!(entry.status, HistoryStatus::Success);
    assert_eq!(entry.text, "done");

    history
        .complete_request_error(&req_id, "boom".to_string())
        .unwrap();
    let entry = history.get_by_id(&req_id).unwrap().expect("entry");
    assert_eq!(entry.status, HistoryStatus::Error);
    assert_eq!(entry.error_message.as_deref(), Some("boom"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_history_prune_older_than() {
    let dir = make_temp_dir("history-prune");
    let history = HistoryStorage::new(dir.clone());

    let entry = history.add_entry("old".to_string(), Some(10)).unwrap();

    let cutoff = Utc::now() + ChronoDuration::seconds(1);
    let removed = history.prune_older_than(cutoff).unwrap();
    assert_eq!(removed, vec![entry.id]);
    assert_eq!(history.get_all(None).unwrap().len(), 0);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_request_history_update_api_keeps_request_metadata_together() {
    let dir = make_temp_dir("history-request-update");
    let history = HistoryStorage::new(dir.clone());
    let req_id = "req-lifecycle".to_string();

    history
        .apply_request_update(RequestHistoryUpdate::CreateInProgress {
            request_id: req_id.clone(),
            model_info: RequestModelInfo {
                stt_provider: Some("groq".to_string()),
                stt_model: Some("whisper-large-v3".to_string()),
                llm_provider: Some("openai".to_string()),
                llm_model: Some("gpt-4o-mini".to_string()),
                profile_id: None,
                profile_name: None,
                preset_id: None,
                preset_name: None,
            },
            max_entries: Some(10),
        })
        .unwrap();

    history
        .apply_request_update(RequestHistoryUpdate::SetProfile {
            request_id: req_id.clone(),
            profile_id: Some("profile-1".to_string()),
            profile_name: Some("Terminal".to_string()),
        })
        .unwrap();
    history
        .apply_request_update(RequestHistoryUpdate::SetPreset {
            request_id: req_id.clone(),
            preset_id: Some("preset-1".to_string()),
            preset_name: Some("Brevity".to_string()),
        })
        .unwrap();
    history
        .apply_request_update(RequestHistoryUpdate::SetRecordingSource {
            request_id: req_id.clone(),
            recording_request_id: Some("rec-1".to_string()),
        })
        .unwrap();
    history
        .apply_request_update(RequestHistoryUpdate::SetLlmModel {
            request_id: req_id.clone(),
            llm_provider: Some("anthropic".to_string()),
            llm_model: Some("claude-test".to_string()),
        })
        .unwrap();
    history
        .apply_request_update(RequestHistoryUpdate::CompleteSuccess {
            request_id: req_id.clone(),
            text: "finished".to_string(),
        })
        .unwrap();

    let entry = history.get_by_id(&req_id).unwrap().expect("entry");
    assert_eq!(entry.status, HistoryStatus::Success);
    assert_eq!(entry.text, "finished");
    assert_eq!(entry.profile_id.as_deref(), Some("profile-1"));
    assert_eq!(entry.profile_name.as_deref(), Some("Terminal"));
    assert_eq!(entry.preset_id.as_deref(), Some("preset-1"));
    assert_eq!(entry.preset_name.as_deref(), Some("Brevity"));
    assert_eq!(entry.recording_request_id.as_deref(), Some("rec-1"));
    assert_eq!(entry.llm_provider.as_deref(), Some("anthropic"));
    assert_eq!(entry.llm_model.as_deref(), Some("claude-test"));

    history
        .apply_request_update(RequestHistoryUpdate::Delete {
            request_id: req_id.clone(),
        })
        .unwrap();
    assert!(history.get_by_id(&req_id).unwrap().is_none());

    let _ = fs::remove_dir_all(&dir);
}
