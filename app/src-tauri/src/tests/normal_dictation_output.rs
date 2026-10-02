use super::*;

#[cfg(target_os = "linux")]
pub(crate) fn native_output_warning(app: &AppHandle) {
    let logs = app.state::<RequestLogStore>();
    let old = logs.start_request("openai".into(), None);
    record_output_failure(app, Some(&old), "native fixture");
    assert!(logs.get_logs(None)[0]
        .entries
        .iter()
        .any(|entry| entry.message == "Output failed: native fixture"));
    let new = logs.start_request("groq".into(), None);
    record_output_failure(app, Some(&old), "late failure");
    assert_eq!(logs.get_logs(None)[0].id, new);
    assert!(!logs.get_logs(None)[0]
        .entries
        .iter()
        .any(|entry| entry.message.starts_with("Output failed:")));
}

#[test]
fn output_decision_prioritizes_quick_replace_failure() {
    assert_eq!(
        decide_normal_dictation_output(Some("rewrite failed"), true),
        NormalDictationOutputDecision::QuickReplaceFailure
    );
}

#[test]
fn output_decision_skips_when_live_output_completed() {
    assert_eq!(
        decide_normal_dictation_output(None, true),
        NormalDictationOutputDecision::LiveOutputAlreadyCompleted
    );
}

#[test]
fn output_decision_outputs_for_normal_dictation() {
    assert_eq!(
        decide_normal_dictation_output(None, false),
        NormalDictationOutputDecision::Output
    );
}

#[test]
fn output_warning_is_scoped_to_the_request_that_failed() {
    let app = tauri::test::mock_builder()
        .manage(RequestLogStore::new())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let logs = app.state::<RequestLogStore>();
    let old = logs.start_request("openai".into(), None);
    record_output_failure(app.handle(), Some(&old), "test delivery failure");
    assert!(logs.get_logs(None)[0]
        .entries
        .iter()
        .any(|entry| entry.message == "Output failed: test delivery failure"));
    let new = logs.start_request("groq".into(), None);
    record_output_failure(app.handle(), Some(&old), "late failure");
    record_output_failure(app.handle(), None, "unowned failure");
    let current = &logs.get_logs(None)[0];
    assert_eq!(current.id, new);
    assert_eq!(current.status, RequestStatus::InProgress);
    assert!(!current
        .entries
        .iter()
        .any(|entry| entry.message.starts_with("Output failed:")));
    let no_logs = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    record_output_failure(no_logs.handle(), Some(&old), "no log store");
}
