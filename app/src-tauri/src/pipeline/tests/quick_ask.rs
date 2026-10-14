use super::*;
use crate::sessions::{quick_action_execution as execution, quick_action_lifecycle as lifecycle};
use serde_json::{json, Value};
use tauri::{Listener, Manager};
use tauri_plugin_store::StoreExt;
use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

pub(super) async fn native_answers(app: &tauri::AppHandle) {
    let pipeline = app.state::<SharedPipeline>();
    let original = pipeline.config();
    let settings = app.store("settings.json").unwrap();
    let logs = app.state::<crate::request_log::RequestLogStore>();
    let memory = app.state::<crate::state::QuickAskConversationMemory>();
    let mut config = test_config_for_transcription();
    let server = MockServer::start().await;
    config.llm_config.ollama_url = Some(server.uri());
    pipeline.update_config(config).unwrap();
    let (sent, received) = std::sync::mpsc::channel();
    let listener = app.listen(crate::events::EVENT_QUICK_ASK_PARTIAL, move |event| {
        sent.send(serde_json::from_str::<Value>(event.payload()).unwrap())
            .unwrap();
    });
    let (sent, answers) = std::sync::mpsc::channel();
    let final_listener = app.listen(crate::events::EVENT_QUICK_ASK_ANSWER, move |event| {
        sent.send(serde_json::from_str::<Value>(event.payload()).unwrap())
            .unwrap();
    });
    let profile = lifecycle::QuickAskProfileConfig {
        provider: Some("ollama".into()),
        model: Some("fixture".into()),
        ..Default::default()
    };
    let ocr = pipeline.config().ocr_config;
    let result = TranscriptionResult {
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
        llm_outcome: LlmOutcome::NotAttempted(LlmNotAttemptedReason::DisabledByDefaultProfile),
        live_output_completed: false,
    };
    let run = |id, result, profile| {
        execution::answer_quick_ask(execution::QuickAskExecution {
            app,
            pipeline: pipeline.inner(),
            request_id: id,
            result,
            fallback_text: "",
            profile_config: profile,
            probe_plan: lifecycle::QuickActionProbePlan::new(
                lifecycle::QuickActionKind::QuickAsk,
                0,
                crate::commands::text::ContextGrabMethod::None,
                0,
            ),
            ocr_mode: "off",
            ocr_config: &ocr,
        })
    };
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(200).set_body_string(
        format!("{}\n{}\n", json!({"message":{"content":"שלום"},"done":false}),
            json!({"message":{"content":" world"},"done":true,"done_reason":"stop","prompt_eval_count":3,"eval_count":4}))
    )).mount(&server).await;
    let mut buffer = crate::audio_capture::AudioBuffer::new(16000, 1, 1.0);
    buffer.append(&vec![0.25; 16000]);
    pipeline.inner.lock().unwrap().last_wav_bytes = Some(Arc::new(buffer.to_wav_bytes().unwrap()));
    let id = logs.start_request("mock".into(), None);
    settings.set("quick_ask_conversation_history_enabled", true);
    memory.push_turn("previous question".into(), "previous answer".into());
    run(Some(id.as_str()), &result, &profile).await;
    let preview = received.try_recv().unwrap();
    assert_eq!(preview["request_id"], id);
    assert_eq!(preview["seq"], 1);
    assert_eq!(preview["text"], "שלום");
    let answer = answers.try_recv().unwrap();
    assert_eq!(answer["answer"], "שלום world");
    assert_eq!(answer["request_id"], id);
    let log = logs.get_logs(Some(1)).remove(0);
    assert_eq!(log.status, crate::request_log::RequestStatus::Success);
    assert_eq!(log.quick_ask_answer.as_deref(), Some("שלום world"));
    assert_eq!(
        log.quick_ask_response_json.as_ref().unwrap()["prompt_eval_count"],
        3
    );
    assert_eq!(memory.snapshot_last(1)[0].answer, "שלום world");
    assert!(logs.with_current(|_| ()).is_none());
    assert!(
        String::from_utf8(server.received_requests().await.unwrap()[0].body.clone())
            .unwrap()
            .contains("previous answer")
    );

    // An old STT completion cannot claim/cancel an already active newer answer.
    let newer_id = logs.start_request("mock".into(), None);
    let newer = app
        .state::<crate::sessions::quick_ask::QuickAskSessions>()
        .begin(newer_id.clone(), None);
    run(Some(id.as_str()), &result, &profile).await;
    assert!(newer.current());
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    assert_eq!(logs.with_current(|log| log.id.clone()), Some(newer_id));
    drop(newer);
    logs.clear();
    // Ownership changes while context is pending: no model dispatch or newer-log completion.
    let id = logs.start_request("mock".into(), None);
    {
        let state = app.state::<crate::state::AppState>();
        let mut probe = state.quick_ask_probe.lock().unwrap();
        probe.epoch = 49;
        probe.ready = false;
    }
    let mut pending = Box::pin(execution::answer_quick_ask(execution::QuickAskExecution {
        app,
        pipeline: pipeline.inner(),
        request_id: Some(&id),
        result: &result,
        fallback_text: "",
        profile_config: &profile,
        probe_plan: lifecycle::QuickActionProbePlan::new(
            lifecycle::QuickActionKind::QuickAsk,
            49,
            crate::commands::text::ContextGrabMethod::CtrlC,
            5000,
        ),
        ocr_mode: "off",
        ocr_config: &ocr,
    }));
    assert!(futures_util::poll!(pending.as_mut()).is_pending());
    let newer_id = logs.start_request("mock".into(), None);
    app.state::<crate::state::AppState>()
        .quick_ask_probe
        .lock()
        .unwrap()
        .ready = true;
    pending.await;
    assert_eq!(logs.with_current(|log| log.id.clone()), Some(newer_id));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    logs.clear();

    // Cancellation before dispatch archives only the matching request, never starts inference.
    let cancel = CancellationToken::new();
    cancel.cancel();
    pipeline.inner.lock().unwrap().cancel_token = Some(cancel);
    let id = logs.start_request("mock".into(), None);
    run(Some(id.as_str()), &result, &profile).await;
    assert_eq!(
        logs.get_logs(Some(1))[0].status,
        crate::request_log::RequestStatus::Cancelled
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    pipeline.inner.lock().unwrap().cancel_token = None;
    logs.clear();
    // Dismissal during a provider preview drops the owned stream and archives cancellation.
    let id = logs.start_request("mock".into(), None);
    let handle = app.clone();
    let cancel_id = id.clone();
    let cancel_listener = app.listen(crate::events::EVENT_QUICK_ASK_PARTIAL, move |_| {
        crate::sessions::quick_ask::cancel_quick_ask(handle.clone(), cancel_id.clone());
    });
    run(Some(id.as_str()), &result, &profile).await;
    app.unlisten(cancel_listener);
    assert_eq!(
        logs.get_logs(Some(1))[0].status,
        crate::request_log::RequestStatus::Cancelled
    );
    assert!(
        answers.try_recv().is_err(),
        "cancelled answers cannot publish success"
    );
    logs.clear();
    // A newer recording begins during a preview; cleanup cannot archive or mutate it.
    let id = logs.start_request("mock".into(), None);
    let handle = app.clone();
    let replacement_listener = app.listen(crate::events::EVENT_QUICK_ASK_PARTIAL, move |_| {
        handle
            .state::<crate::request_log::RequestLogStore>()
            .start_request("newer".into(), None);
    });
    run(Some(id.as_str()), &result, &profile).await;
    app.unlisten(replacement_listener);
    assert_eq!(
        logs.with_current(|log| log.stt_provider.clone()),
        Some("newer".into())
    );
    assert_eq!(
        logs.with_current(|log| log.status.clone()),
        Some(crate::request_log::RequestStatus::InProgress)
    );
    assert!(answers.try_recv().is_err());
    logs.clear();
    server.reset().await;
    settings.set("quick_ask_conversation_history_enabled", false);
    let before_custom = pipeline.config();
    let mut custom_config = before_custom.clone();
    custom_config
        .llm_api_keys
        .insert("custom_fixture".into(), "synthetic-key".into());
    custom_config.llm_config.custom_providers = vec![serde_json::from_value(json!({
        "id":"custom_fixture","name":"Synthetic endpoint","base_url":server.uri(),
        "stt_models":[],"llm_models":["fixture"]
    }))
    .unwrap()];
    pipeline.update_config(custom_config).unwrap();
    let custom_profile = lifecycle::QuickAskProfileConfig {
        provider: Some("custom_fixture".into()),
        ..profile.clone()
    };
    let body = format!(
        "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
        json!({"choices":[{"index":0,"delta":{"content":"Synthetic answer without usage"}}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]})
    );
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;
    let id = logs.start_request("mock".into(), None);
    run(Some(id.as_str()), &result, &custom_profile).await;
    assert_eq!(
        answers.try_recv().unwrap()["answer"],
        "Synthetic answer without usage"
    );
    assert_eq!(
        logs.get_logs(Some(1))[0].status,
        crate::request_log::RequestStatus::Success
    );
    logs.clear();
    pipeline.update_config(before_custom).unwrap();
    server.reset().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let id = logs.start_request("mock".into(), None);
    run(Some(id.as_str()), &result, &profile).await;
    assert_eq!(answers.try_recv().unwrap()["ok"], false);
    assert_eq!(
        logs.get_logs(Some(1))[0].status,
        crate::request_log::RequestStatus::Error
    );
    logs.clear();
    let bad = lifecycle::QuickAskProfileConfig {
        provider: Some("unsupported-provider".into()),
        ..Default::default()
    };
    let id = logs.start_request("mock".into(), None);
    run(Some(id.as_str()), &result, &bad).await;
    assert_eq!(answers.try_recv().unwrap()["ok"], false);
    assert_eq!(
        logs.get_logs(Some(1))[0].status,
        crate::request_log::RequestStatus::Error
    );
    logs.clear();
    let empty = TranscriptionResult {
        stt_text: "".into(),
        ..result.clone()
    };
    let id = logs.start_request("mock".into(), None);
    run(Some(id.as_str()), &empty, &profile).await;
    assert_eq!(
        answers.try_recv().unwrap()["error"],
        "No transcript to answer (empty)"
    );
    logs.clear();
    pipeline.inner.lock().unwrap().last_wav_bytes = None;
    settings.delete("quick_ask_conversation_history_enabled");
    app.unlisten(listener);
    app.unlisten(final_listener);
    pipeline.update_config(original).unwrap();
}
