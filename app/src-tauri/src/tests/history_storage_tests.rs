use crate::history::{HistoryStatus, HistoryStorage, RequestModelInfo};
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

	let entry = history
		.add_entry("old".to_string(), Some(10))
		.unwrap();

	let cutoff = Utc::now() + ChronoDuration::seconds(1);
	let removed = history.prune_older_than(cutoff).unwrap();
	assert_eq!(removed, vec![entry.id]);
	assert_eq!(history.get_all(None).unwrap().len(), 0);

	let _ = fs::remove_dir_all(&dir);
}
