use super::*;

#[test]
fn a_poisoned_owner_lock_preserves_cancellation_and_replacement_ownership() {
    let sessions = QuickAskSessions::default();
    let answer = sessions.begin("one".into(), None);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = sessions.0.lock().unwrap();
        panic!("synthetic interrupted owner access");
    }));
    assert!(answer.current());
    sessions.cancel("one");
    assert!(!answer.current());
    drop(answer);
    let replacement = sessions.begin("two".into(), None);
    assert!(replacement.current());
    drop(replacement);
}

#[test]
fn replacement_parent_cancellation_and_stale_dismissal_do_not_touch_new_session() {
    let sessions = QuickAskSessions::default();
    sessions.cancel("missing");
    let parent = CancellationToken::new();
    let old = sessions.begin("old".into(), Some(parent.clone()));
    assert!(old.current());
    let current = sessions.begin("new".into(), None);
    assert!(!old.current());
    assert!(old.cancel.is_cancelled());
    assert!(!parent.is_cancelled());
    sessions.cancel("old");
    drop(old);
    assert!(current.current());
    sessions.cancel("new");
    assert!(!current.current());
    drop(current);
    assert!(sessions.0.lock().unwrap().is_none());
    let answer = sessions.begin("parent".into(), Some(parent.clone()));
    parent.cancel();
    assert!(!answer.current());
    drop(answer);
    let first = sessions.begin("same-id".into(), None);
    let second = sessions.begin("same-id".into(), None);
    drop(first);
    assert!(second.current());
    drop(second);
    let detached = sessions.begin("detached".into(), None);
    *sessions.0.lock().unwrap() = None;
    assert!(!detached.current());
    drop(detached);
}

#[test]
fn scope_only_mutates_and_completes_its_own_request() {
    use crate::request_log::RequestLogStore;
    let store = RequestLogStore::new();
    let old = store.start_request("mock".into(), None);
    let scope = store.for_request(&old);
    scope
        .with_current(|log| log.quick_ask_answer = Some("answer".into()))
        .unwrap();
    assert_eq!(
        store.get_logs(Some(1))[0].quick_ask_answer.as_deref(),
        Some("answer")
    );
    assert!(scope.with_current_id(&old, |_| ()).is_some());
    let newer = store.start_request("mock".into(), None);
    assert!(scope.with_current(|_| panic!("stale callback")).is_none());
    assert!(scope
        .with_current_id(&newer, |_| panic!("scope cannot adopt another request"))
        .is_none());
    scope.complete_current();
    assert_eq!(
        store.with_current(|log| log.id.clone()),
        Some(newer.clone())
    );
    store.for_request(&newer).complete_current();
    scope.complete_current();
    assert!(store.with_current(|_| ()).is_none());
    assert!(scope.with_current_id(&old, |_| ()).is_none());
    let background = scope.fork_current();
    let id = background.start_request("background".into(), None);
    assert!(background.with_current_id(&id, |_| ()).is_some());
}

pub(crate) fn native_cancel(app: &AppHandle) {
    let session = app.state::<QuickAskSessions>().begin("native".into(), None);
    cancel_quick_ask(app.clone(), "stale".into());
    assert!(session.current());
    cancel_quick_ask(app.clone(), "native".into());
    assert!(!session.current());
    drop(session);
    // The minimal fixture intentionally has no log store at this stage. A missing
    // provider key must still produce a recoverable answer error without an account/log.
    assert!(app
        .try_state::<crate::request_log::RequestLogStore>()
        .is_none());
    use crate::sessions::{
        quick_action_execution as execution, quick_action_lifecycle as lifecycle,
    };
    let pipeline = app.state::<crate::pipeline::SharedPipeline>();
    let ocr = pipeline.config().ocr_config;
    let result = crate::pipeline::TranscriptionResult {
        stt_language_hint: None,
        stt_detected_languages: vec![],
        speaker_segments: vec![],
        stt_text: "Synthetic question".into(),
        final_text: "Synthetic question".into(),
        stt_duration_ms: 0,
        stt_retry: None,
        llm_duration_ms: None,
        llm_provider_used: None,
        llm_model_used: None,
        llm_outcome: crate::pipeline::LlmOutcome::NotAttempted(
            crate::pipeline::LlmNotAttemptedReason::DisabledByDefaultProfile,
        ),
        live_output_completed: false,
    };
    let profile = lifecycle::QuickAskProfileConfig {
        provider: Some("missing-provider".into()),
        ..Default::default()
    };
    use tauri::Listener;
    let (sent, received) = std::sync::mpsc::channel();
    let listener = app.listen(EVENT_QUICK_ASK_ANSWER, move |event| {
        sent.send(event.payload().to_string()).unwrap();
    });
    tauri::async_runtime::block_on(execution::answer_quick_ask(execution::QuickAskExecution {
        app,
        pipeline: pipeline.inner(),
        request_id: None,
        result: &result,
        fallback_text: "",
        profile_config: &profile,
        probe_plan: lifecycle::QuickActionProbePlan::new(
            lifecycle::QuickActionKind::QuickAsk,
            0,
            crate::commands::text::ContextGrabMethod::None,
            0,
        ),
        ocr_mode: "off",
        ocr_config: &ocr,
    }));
    let answer: serde_json::Value = serde_json::from_str(&received.try_recv().unwrap()).unwrap();
    assert_eq!(answer["ok"], false);
    assert!(answer["error"].as_str().unwrap().contains("API key"));
    app.unlisten(listener);
}
