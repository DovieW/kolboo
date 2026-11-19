use super::*;
use serde_json::json;

#[test]
fn recording_context_without_quick_ask_state_is_safe_and_a_managed_context_preserves_ownership() {
    use tauri::test::{mock_builder, mock_context, noop_assets};
    let app = mock_builder().build(mock_context(noop_assets())).unwrap();
    assert!(recording_request_id(app.try_state::<QuickAskSessions>()).is_none());
    app.manage(QuickAskSessions::default());
    let current = app
        .state::<QuickAskSessions>()
        .begin("headless-recording".into(), None);
    assert_eq!(
        recording_request_id(app.try_state::<QuickAskSessions>()).as_deref(),
        Some("headless-recording")
    );
    assert!(current.current());
    drop(current);
}

#[test]
fn replay_keeps_one_latest_request_snapshot_and_survives_terminal_lease_drop() {
    let sessions = QuickAskSessions::default();
    assert!(sessions.snapshot().is_none());
    let old = sessions.begin("one".into(), None);
    {
        let mut surface = sessions.1.lock().unwrap();
        assert!(surface.update(EVENT_QUICK_ASK_STARTED, json!({})).is_none());
        assert!(surface
            .update(EVENT_QUICK_ASK_STARTED, json!({"request_id":"stale"}))
            .is_none());
        assert!(surface
            .update(
                EVENT_QUICK_ASK_STARTED,
                json!({"request_id":"one","question":false})
            )
            .is_none());
        assert!(surface
            .update(
                events::EVENT_QUICK_ASK_PARTIAL,
                json!({"request_id":"one","seq":1,"text":"before start"})
            )
            .is_none());
        assert!(surface
            .update(
                EVENT_QUICK_ASK_ANSWER,
                json!({"request_id":"one","ok":true,"answer":"before start"})
            )
            .is_none());
        let started = surface
            .update(
                EVENT_QUICK_ASK_STARTED,
                json!({"request_id":"one","question":"Synthetic question"}),
            )
            .unwrap();
        assert_eq!(started.revision, 2);
        assert_eq!(
            started.started.unwrap().question.as_deref(),
            Some("Synthetic question")
        );
        assert!(surface
            .update("unknown", json!({"request_id":"one"}))
            .is_none());
        assert!(surface
            .update(
                events::EVENT_QUICK_ASK_PARTIAL,
                json!({"request_id":"one","seq":"bad","text":"invalid"})
            )
            .is_none());
        assert!(surface
            .update(
                EVENT_QUICK_ASK_ANSWER,
                json!({"request_id":"one","ok":true})
            )
            .is_none());
        let partial = surface
            .update(
                events::EVENT_QUICK_ASK_PARTIAL,
                json!({"request_id":"one","seq":1,"text":"first"}),
            )
            .unwrap();
        assert_eq!(partial.partial.unwrap().text, "first");
        assert!(surface
            .update(
                events::EVENT_QUICK_ASK_PARTIAL,
                json!({"request_id":"one","seq":1,"text":"duplicate"})
            )
            .is_none());
        let partial = surface
            .update(
                events::EVENT_QUICK_ASK_PARTIAL,
                json!({"request_id":"one","seq":3,"text":"latest"}),
            )
            .unwrap();
        assert_eq!(partial.partial.unwrap().text, "latest");
        let answer = surface
            .update(
                EVENT_QUICK_ASK_ANSWER,
                json!({"request_id":"one","ok":false,"error":"Synthetic failure"}),
            )
            .unwrap();
        assert_eq!(answer.revision, 5);
        assert_eq!(answer.partial.unwrap().text, "latest");
        assert!(surface
            .update(
                events::EVENT_QUICK_ASK_PARTIAL,
                json!({"request_id":"one","seq":4,"text":"late"})
            )
            .is_none());
        assert!(surface
            .update(
                EVENT_QUICK_ASK_ANSWER,
                json!({"request_id":"one","ok":true,"answer":"late"})
            )
            .is_none());
    }
    drop(old);
    assert!(
        sessions.snapshot().unwrap().answer.is_some(),
        "completed content remains available while displayed"
    );
    sessions.cancel("stale");
    assert!(sessions.snapshot().is_some());
    sessions.cancel("one");
    assert!(
        sessions.snapshot().unwrap().started.is_none(),
        "dismissal clears sensitive terminal content too"
    );
    let new = sessions.begin("two".into(), None);
    let state = sessions
        .1
        .lock()
        .unwrap()
        .update(EVENT_QUICK_ASK_STARTED, json!({"request_id":"two"}))
        .unwrap();
    assert_eq!(
        state.revision, 8,
        "revisions remain monotonic across replacement"
    );
    drop(new);
    assert!(
        sessions.snapshot().unwrap().started.is_none(),
        "an abandoned stream cannot replay as active"
    );
}

#[test]
fn replacement_clears_sensitive_snapshots_before_a_new_start_and_stale_drop_is_harmless() {
    let sessions = QuickAskSessions::default();
    let old = sessions.begin("old".into(), None);
    sessions
        .1
        .lock()
        .unwrap()
        .update(
            EVENT_QUICK_ASK_STARTED,
            json!({"request_id":"old","question":"old question"}),
        )
        .unwrap();
    let current = sessions.begin("current".into(), None);
    assert!(sessions.snapshot().unwrap().started.is_none());
    drop(old);
    sessions
        .1
        .lock()
        .unwrap()
        .update(EVENT_QUICK_ASK_STARTED, json!({"request_id":"current"}))
        .unwrap();
    assert!(current.current());
    assert_eq!(
        sessions
            .snapshot()
            .unwrap()
            .started
            .unwrap()
            .request_id
            .as_deref(),
        Some("current")
    );
    drop(current);
}

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

#[cfg(target_os = "linux")]
pub(crate) fn native_cancel(app: &AppHandle) {
    struct Unserializable;
    impl serde::Serialize for Unserializable {
        fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("synthetic serialization failure"))
        }
    }
    emit_to_quick_ask(app, EVENT_QUICK_ASK_STARTED, Unserializable);
    let session = app.state::<QuickAskSessions>().begin("native".into(), None);
    // Exercise recovery through the real emitter, GUI lifecycle and IPC paths,
    // not only an isolated state instance. No private content is in the panic.
    for surface in [false, true] {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let sessions = app.state::<QuickAskSessions>();
            if surface {
                let _guard = sessions.1.lock().unwrap();
                panic!("synthetic interrupted surface access");
            } else {
                let _guard = sessions.0.lock().unwrap();
                panic!("synthetic interrupted answer access");
            }
        }));
    }
    let unsupported_event = "unknown";
    emit_to_quick_ask(app, unsupported_event, json!({"request_id":"native"}));
    emit_to_quick_ask(
        app,
        EVENT_QUICK_ASK_ANSWER,
        json!({"request_id":"stale","ok":true,"answer":"stale"}),
    );
    cancel_quick_ask(app.clone(), "stale".into(), None);
    assert!(session.current());
    cancel_quick_ask(app.clone(), "native".into(), None);
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
    execution::complete_quick_ask_empty_transcript_error(app, pipeline.inner(), None);
    let replay = app.state::<QuickAskSessions>().snapshot().unwrap();
    assert!(replay.started.unwrap().request_id.is_some());
    assert!(
        matches!(replay.answer, Some(crate::QuickAskAnswerPayload::Err(error)) if error.error == "No transcript to answer (empty)")
    );
}

#[cfg(target_os = "linux")]
#[allow(deprecated)] // Isolated native runner pumps real GUI work; no default-test sleeps.
pub(crate) fn native_lazy_lifecycle(app: &mut tauri::App) {
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};

    fn pump(app: &mut tauri::App, predicate: impl Fn(&tauri::App) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(8);
        while !predicate(app) {
            assert!(
                Instant::now() < deadline,
                "Quick Ask native lifecycle stalled"
            );
            app.run_iteration(|_, event| {
                if let tauri::RunEvent::ExitRequested { api, .. } = event {
                    api.prevent_exit();
                }
            });
        }
    }
    fn settled(app: &tauri::App) -> bool {
        !app.state::<QuickAskSessions>()
            .1
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .presenting
    }
    recording_started(app.handle());
    pump(app, |app| {
        app.get_webview_window(QUICK_ASK_WINDOW_LABEL).is_none()
            && settled(app)
            && !app
                .state::<crate::state::AppState>()
                .quick_ask_window_creating
                .load(Ordering::SeqCst)
    });
    let initial_revision = app.state::<QuickAskSessions>().snapshot().unwrap().revision;
    app.state::<QuickAskSessions>()
        .1
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .presenting = true;
    complete_presentation_dispatch(app.handle(), Ok(()));
    assert!(
        !settled(app),
        "successful dispatch leaves completion ownership with its queued GUI callback"
    );
    complete_presentation_dispatch(
        app.handle(),
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe).into()),
    );
    assert!(
        settled(app),
        "failed GUI dispatch releases the presentation lease"
    );
    assert_eq!(
        app.state::<QuickAskSessions>().snapshot().unwrap().revision,
        initial_revision
    );
    complete_presentation(app.handle(), None, true, None);
    assert!(
        settled(app),
        "failed construction with no registered window can retry"
    );
    let (registered, resume) = crate::overlay::tests::pause_next_quick_ask_configuration();
    let old = app
        .state::<QuickAskSessions>()
        .begin("native-cold-old".into(), None);
    emit_to_quick_ask(
        app.handle(),
        EVENT_QUICK_ASK_STARTED,
        json!({"request_id":"native-cold-old","question":"Old synthetic question"}),
    );
    pump(app, |_| registered.try_recv().is_ok());
    assert!(
        app.get_webview_window(QUICK_ASK_WINDOW_LABEL).is_some(),
        "barrier follows real Wry registration"
    );
    assert!(app
        .state::<crate::state::AppState>()
        .quick_ask_window_creating
        .load(Ordering::SeqCst));
    cancel_quick_ask(app.handle().clone(), "native-cold-old".into(), Some(true));
    assert!(!old.current());
    let answer = app
        .state::<QuickAskSessions>()
        .begin("native-lazy".into(), None);
    emit_to_quick_ask(
        app.handle(),
        EVENT_QUICK_ASK_STARTED,
        json!({"request_id":"native-lazy","question":"Synthetic question"}),
    );
    assert!(
        !settled(app),
        "the replacement coalesces into the in-flight creator"
    );
    drop(old);
    emit_to_quick_ask(
        app.handle(),
        events::EVENT_QUICK_ASK_PARTIAL,
        json!({"request_id":"native-lazy","seq":1,"text":"Synthetic partial"}),
    );
    emit_to_quick_ask(
        app.handle(),
        EVENT_QUICK_ASK_ANSWER,
        json!({"request_id":"native-lazy","ok":true,"answer":"Synthetic answer"}),
    );
    drop(answer);
    resume.send(()).unwrap();
    pump(app, settled);
    assert!(
        app.get_webview_window(QUICK_ASK_WINDOW_LABEL)
            .unwrap()
            .is_visible()
            .unwrap(),
        "the latest owned answer reaches a valid, visible native window"
    );
    let replay = tauri::async_runtime::block_on(
        crate::commands::overlay::quick_ask_frontend_ready(app.handle().clone()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        replay.started.unwrap().question.as_deref(),
        Some("Synthetic question")
    );
    assert!(
        replay.answer.is_some(),
        "fast completion survives cold renderer startup"
    );
    let handle = app.handle().clone();
    let (sent, received) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let window = crate::overlay::ensure_quick_ask_window(&handle).unwrap();
        sent.send(window.label().to_string()).unwrap();
    });
    pump(app, |_| worker.is_finished());
    worker.join().unwrap();
    assert_eq!(received.try_recv().unwrap(), QUICK_ASK_WINDOW_LABEL);

    // begin() precedes its started payload. An older creator completing in
    // that gap must not mistake the live successor for a cancellation.
    let pending = app
        .state::<QuickAskSessions>()
        .begin("native-pending".into(), None);
    complete_presentation(
        app.handle(),
        app.get_webview_window(QUICK_ASK_WINDOW_LABEL),
        false,
        Some("native-lazy"),
    );
    assert!(pending.current());
    assert!(app
        .get_webview_window(QUICK_ASK_WINDOW_LABEL)
        .unwrap()
        .is_visible()
        .unwrap());
    emit_to_quick_ask(
        app.handle(),
        EVENT_QUICK_ASK_STARTED,
        json!({"request_id":"native-pending"}),
    );
    emit_to_quick_ask(
        app.handle(),
        EVENT_QUICK_ASK_ANSWER,
        json!({"request_id":"native-pending","ok":true,"answer":"Synthetic successor answer"}),
    );
    drop(pending);
    pump(app, settled);

    // A real destroyed Wry handle fails positioning/show. The fallback/error
    // completion must release ownership without discarding the displayed answer.
    let stale = app.get_webview_window(QUICK_ASK_WINDOW_LABEL).unwrap();
    stale.destroy().unwrap();
    pump(app, |app| {
        app.get_webview_window(QUICK_ASK_WINDOW_LABEL).is_none()
    });
    assert!(position_presentation_window(app.handle(), &stale));
    app.state::<QuickAskSessions>()
        .1
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .presenting = true;
    complete_presentation(app.handle(), Some(stale), false, Some("native-lazy"));
    assert!(settled(app));
    assert!(app
        .state::<QuickAskSessions>()
        .snapshot()
        .unwrap()
        .answer
        .is_some());
    // If a prior construction failed while a replacement coalesced into it,
    // finalization transfers exactly one fresh attempt to that replacement.
    app.state::<QuickAskSessions>()
        .1
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .presenting = true;
    complete_presentation(app.handle(), None, true, Some("failed-predecessor"));
    assert!(!settled(app));
    pump(app, settled);
    assert!(
        app.get_webview_window(QUICK_ASK_WINDOW_LABEL)
            .unwrap()
            .is_visible()
            .unwrap(),
        "a native presentation failure permits a fresh, visible retry"
    );

    let current = app
        .state::<QuickAskSessions>()
        .begin("native-new".into(), None);
    emit_to_quick_ask(
        app.handle(),
        EVENT_QUICK_ASK_STARTED,
        json!({"request_id":"native-new"}),
    );
    cancel_quick_ask(app.handle().clone(), "native-lazy".into(), Some(true));
    assert!(current.current());
    assert!(app.get_webview_window(QUICK_ASK_WINDOW_LABEL).is_some());
    let before = app.state::<QuickAskSessions>().snapshot().unwrap().revision;
    recording_started(app.handle());
    assert!(!current.current());
    let tombstone = app.state::<QuickAskSessions>().snapshot().unwrap();
    assert!(tombstone.revision > before);
    assert!(
        tombstone.started.is_none() && tombstone.partial.is_none() && tombstone.answer.is_none()
    );
    emit_to_quick_ask(
        app.handle(),
        EVENT_QUICK_ASK_ANSWER,
        json!({"request_id":"native-new","ok":true,"answer":"must not replay"}),
    );
    assert!(app
        .state::<QuickAskSessions>()
        .snapshot()
        .unwrap()
        .started
        .is_none());
    drop(current);
    pump(app, |app| {
        app.get_webview_window(QUICK_ASK_WINDOW_LABEL).is_none()
            && settled(app)
            && !app
                .state::<crate::state::AppState>()
                .quick_ask_window_creating
                .load(Ordering::SeqCst)
    });
    // No owner/no window, repeated recording-start invalidation is harmless.
    recording_started(app.handle());
}
