use super::*;
use crate::history::{HistoryStatus, RequestModelInfo};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tauri::Listener;

#[cfg(target_os = "linux")]
pub(crate) fn native_superseded_history(app: &AppHandle) {
    let logs = app.state::<RequestLogStore>();
    let history = app.state::<HistoryStorage>();
    let old = logs.start_request("openai".into(), None);
    history
        .add_request_entry(old.clone(), RequestModelInfo::default(), None)
        .unwrap();
    complete_superseded_request(app, Some(&old), Ok("native fixture")).unwrap();
    complete_superseded_request(app, Some(&old), Ok("not overwritten")).unwrap();
    assert_eq!(
        history.get_by_id(&old).unwrap().unwrap().text,
        "native fixture"
    );
    let new = logs.start_request("groq".into(), None);
    complete_superseded_request(app, Some(&old), Err("late failure")).unwrap();
    assert_eq!(logs.get_logs(None)[0].id, new);
    assert_eq!(logs.get_logs(None)[0].status, RequestStatus::InProgress);
}

#[test]
fn superseded_result_finishes_only_its_row_and_never_emits_pipeline_events() {
    let directory = tempfile::tempdir().unwrap();
    let history = HistoryStorage::new(directory.path().into());
    let logs = RequestLogStore::new();
    let old = logs.start_request("openai".into(), None);
    let new = logs.start_request("groq".into(), None);
    for id in [&old, &new] {
        history
            .add_request_entry(id.clone(), RequestModelInfo::default(), None)
            .unwrap();
    }
    let app = tauri::test::mock_builder()
        .manage(history)
        .manage(logs)
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let notifications = Arc::new(AtomicUsize::new(0));
    let count = notifications.clone();
    app.listen(events::EVENT_HISTORY_CHANGED, move |_| {
        count.fetch_add(1, Ordering::SeqCst);
    });
    let pipeline_events = Arc::new(AtomicUsize::new(0));
    let count = pipeline_events.clone();
    app.listen(events::EVENT_PIPELINE_STATE_CHANGED, move |_| {
        count.fetch_add(1, Ordering::SeqCst);
    });
    complete_superseded_request(app.handle(), Some(&old), Ok("preserved result")).unwrap();
    let history = app.state::<HistoryStorage>();
    let row = history.get_by_id(&old).unwrap().unwrap();
    assert_eq!(row.status, HistoryStatus::Success);
    assert_eq!(row.text, "preserved result");
    assert_eq!(
        history.get_by_id(&new).unwrap().unwrap().status,
        HistoryStatus::InProgress
    );
    assert_eq!(app.state::<RequestLogStore>().get_logs(None)[0].id, new);
    assert_eq!(
        app.state::<RequestLogStore>().get_logs(None)[0].status,
        RequestStatus::InProgress
    );
    assert_eq!(pipeline_events.load(Ordering::SeqCst), 0);
    assert_eq!(notifications.load(Ordering::SeqCst), 1);
    complete_superseded_request(app.handle(), Some(&old), Err("late error")).unwrap();
    complete_superseded_request(app.handle(), None, Ok("unowned")).unwrap();
    history.delete(&old).unwrap();
    complete_superseded_request(app.handle(), Some(&old), Ok("deleted result")).unwrap();
    assert!(history.get_by_id(&old).unwrap().is_none());
    assert_eq!(notifications.load(Ordering::SeqCst), 1);
    assert_eq!(
        HistoryStorage::new(directory.path().into())
            .get_by_id(&new)
            .unwrap()
            .unwrap()
            .status,
        HistoryStatus::InProgress
    );
}

#[test]
fn superseded_request_handles_cancellation_and_missing_stores() {
    let empty = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    complete_superseded_request(empty.handle(), Some("absent"), Err("cancelled")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let history = HistoryStorage::new(directory.path().into());
    let logs = RequestLogStore::new();
    let id = logs.start_request("openai".into(), None);
    history
        .add_request_entry(id.clone(), RequestModelInfo::default(), None)
        .unwrap();
    let app = tauri::test::mock_builder()
        .manage(history)
        .manage(logs)
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    complete_superseded_request(app.handle(), Some(&id), Err("superseded")).unwrap();
    complete_superseded_request(app.handle(), Some(&id), Err("repeat")).unwrap();
    let row = app
        .state::<HistoryStorage>()
        .get_by_id(&id)
        .unwrap()
        .unwrap();
    assert_eq!(row.status, HistoryStatus::Error);
    assert_eq!(row.error_message.as_deref(), Some("superseded"));
    assert_eq!(
        app.state::<RequestLogStore>().get_logs(None)[0].status,
        RequestStatus::Cancelled
    );
}

#[test]
fn superseded_history_save_failure_remains_recoverable_and_emits_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let history = HistoryStorage::new(directory.path().into());
    history
        .add_request_entry("old".into(), RequestModelInfo::default(), None)
        .unwrap();
    let app = tauri::test::mock_builder()
        .manage(history)
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    // Block the final atomic rename using only this test's own temporary files.
    let file = directory.path().join("history.json");
    std::fs::rename(&file, directory.path().join("snapshot.json")).unwrap();
    std::fs::create_dir(&file).unwrap();
    let notifications = Arc::new(AtomicUsize::new(0));
    let count = notifications.clone();
    app.listen(events::EVENT_HISTORY_CHANGED, move |_| {
        count.fetch_add(1, Ordering::SeqCst);
    });
    assert!(complete_superseded_request(app.handle(), Some("old"), Ok("result")).is_err());
    assert_eq!(
        app.state::<HistoryStorage>()
            .get_by_id("old")
            .unwrap()
            .unwrap()
            .status,
        HistoryStatus::InProgress
    );
    assert_eq!(notifications.load(Ordering::SeqCst), 0);
    std::fs::remove_dir(&file).unwrap();
    std::fs::rename(directory.path().join("snapshot.json"), &file).unwrap();
    complete_superseded_request(app.handle(), Some("old"), Ok("result")).unwrap();
    assert_eq!(
        HistoryStorage::new(directory.path().into())
            .get_by_id("old")
            .unwrap()
            .unwrap()
            .text,
        "result"
    );
    assert_eq!(notifications.load(Ordering::SeqCst), 1);
}
