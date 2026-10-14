//! Pipeline integration tests.
//!
//! These tests exercise the recording pipeline without requiring real hardware (CPAL audio
//! devices) or network (STT/LLM API calls) by injecting fake/mock implementations.

use super::ocr_session_state::OcrTaskHandle;
use super::*;
use crate::audio_capture::{
    AudioCaptureBackend, AudioCaptureDiagnostics, AudioCaptureError, AudioCaptureEvent,
    AudioEncodeConfig, AudioLevelSnapshot, AudioLevelStats, SharedAudioLevelMeter,
    SharedAudioWaveformMeter,
};
use crate::stt::AudioFormat;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

const MOCK_PROVIDER: &str = "mock";
const MOCK_API_KEY: &str = "test-key";
mod quick_ask;

impl SharedPipeline {
    pub(crate) fn set_runtime_state_for_native_test(&self, state: PipelineState) {
        self.inner.lock().unwrap().state = state;
    }
    pub(crate) fn poison_runtime_for_native_test(&self) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = self.inner.lock().unwrap();
            panic!("synthetic interrupted runtime access");
        }));
    }
    pub(crate) fn recover_runtime_for_native_test(&self) {
        self.inner.clear_poison();
    }
}

#[tokio::test]
async fn meeting_uploads_receive_owned_job_context_and_completed_chunks_are_not_resubmitted() {
    use crate::recordings::options::{MeetingModel, RecordingMode, RecordingPreferences};
    use crate::stt::{SttTranscript, SttUploadContext, UploadPurpose};
    struct UploadedJob {
        paths: Arc<std::sync::Mutex<Vec<std::path::PathBuf>>>,
    }
    #[async_trait]
    impl SttProvider for UploadedJob {
        fn name(&self) -> &'static str {
            "yiddishlabs"
        }
        async fn transcribe(
            &self,
            _: &[u8],
            _: &AudioFormat,
        ) -> Result<String, crate::stt::SttError> {
            Err(crate::stt::SttError::Config(
                "Dictation must not submit an asynchronous job".into(),
            ))
        }
        async fn transcribe_upload(
            &self,
            audio: &[u8],
            _: &AudioFormat,
            context: &SttUploadContext,
        ) -> Result<SttTranscript, crate::stt::SttError> {
            assert_eq!(context.purpose, UploadPurpose::Meeting);
            assert!(audio.starts_with(b"RIFF"));
            assert!(context.checkpoint_path.parent().unwrap().is_dir());
            self.paths
                .lock()
                .unwrap()
                .push(context.checkpoint_path.clone());
            Ok(SttTranscript {
                text: "שלום — hello".into(),
                segments: Vec::new(),
            })
        }
    }
    let mut config = test_config_for_transcription();
    config.llm_config.enabled = false;
    let pipeline = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    let paths = Arc::new(std::sync::Mutex::new(Vec::new()));
    pipeline.inject_stt_provider_for_tests(
        "yiddishlabs",
        Some(crate::stt::yiddishlabs::STANDARD_MODEL),
        None,
        Arc::new(UploadedJob {
            paths: paths.clone(),
        }),
    );
    let mut buffer = crate::audio_capture::AudioBuffer::new(16000, 1, 1.0);
    buffer.append(&vec![0.25; 16000]);
    let wav = buffer.to_wav_bytes().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let checkpoint = directory.path().join("meeting.transcripts");
    let options = RecordingPreferences {
        mode: RecordingMode::Meeting,
        meeting_model: Some(MeetingModel {
            provider: "yiddishlabs".into(),
            model: crate::stt::yiddishlabs::STANDARD_MODEL.into(),
            use_managed: false,
        }),
        language_hint: None,
    };
    for _ in 0..2 {
        pipeline.begin_recovery().unwrap();
        let result = pipeline
            .transcribe_journal_wav(wav.clone(), None, Some(&checkpoint), &options)
            .await
            .unwrap();
        assert_eq!(result.final_text, "שלום — hello");
        assert_eq!(pipeline.state(), PipelineState::Idle);
        pipeline.end_recovery();
    }
    let bad_directory = tempfile::tempdir().unwrap();
    let bad_checkpoint = bad_directory.path().join("meeting.transcripts");
    std::fs::write(
        bad_checkpoint.with_extension("yiddish-jobs"),
        b"not a job directory",
    )
    .unwrap();
    pipeline.begin_recovery().unwrap();
    let error = pipeline
        .transcribe_journal_wav(wav, None, Some(&bad_checkpoint), &options)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("checkpoint could not be verified"));
    pipeline.end_recovery();
    let paths = paths.lock().unwrap();
    assert_eq!(paths.len(), 1);
    assert_eq!(
        paths[0].parent(),
        Some(checkpoint.with_extension("yiddish-jobs").as_path())
    );
    assert_eq!(
        checkpoint
            .with_extension("yiddish-jobs")
            .read_dir()
            .unwrap()
            .count(),
        0
    );
}

#[tokio::test]
async fn rewrite_renews_only_the_same_owners_token_and_keeps_the_recorded_gateway() {
    use crate::model_selection::{ModelSelection, ModelSource};
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };
    let token = |subject, issued| {
        format!(
            "header.{}.signature",
            URL_SAFE_NO_PAD.encode(
                serde_json::to_vec(
                    &serde_json::json!({"sub":subject,"iss":"synthetic-issuer","iat":issued})
                )
                .unwrap()
            )
        )
    };
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"choices":[{"message":{"content":"synthetic rewrite"}}]}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let old_token = token("original-owner", 1);
    let renewed_token = token("original-owner", 2);
    let mut config = test_config_for_transcription();
    config.managed_inference_enabled = true;
    config.managed_inference_gateway_url = Some(server.uri());
    config.managed_inference_access_token = Some(old_token);
    let store = crate::request_log::RequestLogStore::new();
    store.start_request("synthetic recording".into(), None);
    config.request_log_store = Some(store.clone());
    let pipeline = SharedPipeline::new_for_tests(config.clone(), Box::new(FakeAudioCapture::new()));
    let params = LlmProviderParams {
        selection: Some(ModelSelection {
            source: ModelSource::Managed,
            provider_id: "groq".into(),
            model_id: "synthetic-model".into(),
        }),
        model: None,
        timeout: Duration::from_secs(1),
        ollama_url: None,
        openai_reasoning_effort: None,
        gemini_thinking_budget: None,
        gemini_thinking_level: None,
        anthropic_thinking_budget: None,
    };
    {
        let mut inner = pipeline.inner.lock().unwrap();
        inner.recording_config = Some(config.clone());
        inner.config.managed_inference_access_token = Some(renewed_token.clone());
        inner.config.managed_inference_gateway_url = Some("http://must-not-be-used.invalid".into());
    }
    let provider = pipeline
        .inner
        .lock()
        .unwrap()
        .get_or_create_llm_provider("managed", params.clone())
        .unwrap();
    assert_eq!(
        provider
            .complete("synthetic system", "synthetic input")
            .await
            .unwrap(),
        "synthetic rewrite"
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].headers["authorization"],
        format!("Bearer {renewed_token}")
    );
    assert_eq!(requests[0].headers["x-kolboo-model-provider"], "groq");
    assert_eq!(
        requests[0].body_json::<serde_json::Value>().unwrap()["model"],
        "synthetic-model"
    );
    assert!(store.get_logs(None)[0].managed_inference);
    for current in [None, Some(token("different-owner", 3))] {
        let mut inner = pipeline.inner.lock().unwrap();
        inner.config.managed_inference_access_token = current;
        assert!(
            matches!(inner.get_or_create_llm_provider("managed", params.clone()), Err(PipelineError::Config(message)) if message.contains("session changed"))
        );
    }
    let mut inner = pipeline.inner.lock().unwrap();
    inner.config.managed_inference_access_token = Some(renewed_token);
    inner.config.managed_inference_enabled = false;
    assert!(inner.get_or_create_llm_provider("managed", params).is_err());
}

#[tokio::test]
async fn byok_auth_failure_never_enters_managed_session_refresh() {
    let mut config = test_config_for_transcription();
    config.managed_inference_enabled = true;
    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    let mut provider = MockSttProvider::new("must not succeed");
    provider.behavior.error = Some("401 auth_invalid_token".into());
    let result = p
        .run_batch_stt_request(
            Arc::new(provider),
            MOCK_PROVIDER,
            None,
            None,
            None,
            &[1, 2, 3],
            &crate::stt::RetryConfig::default(),
            Duration::from_secs(1),
            &CancellationToken::new(),
            "test",
            "retry",
            None,
        )
        .await;
    assert!(
        matches!(result, Err(PipelineError::Stt(crate::stt::SttError::Api(message))) if message == "401 auth_invalid_token")
    );
    assert_eq!(p.state(), PipelineState::Error);
}

#[tokio::test]
async fn changing_managed_login_never_blocks_byok_or_sends_with_another_accounts_session() {
    use crate::model_selection::{ModelAction, ModelSelection, ModelSource};
    for source in [ModelSource::Byok, ModelSource::Managed] {
        let mut config = test_config_for_transcription();
        config.llm_config.enabled = false;
        config.managed_inference_enabled = true;
        config.managed_stt_preferred = true;
        config.managed_inference_gateway_url = Some("https://gateway.example.test".into());
        config.managed_inference_access_token = Some("old-session".into());
        config.model_selections.insert(
            ModelAction::Transcription,
            ModelSelection {
                source,
                provider_id: "groq".into(),
                model_id: "whisper-large-v3-turbo".into(),
            },
        );
        let p = SharedPipeline::new_for_tests(config.clone(), Box::new(FakeAudioCapture::new()));
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut provider = MockSttProvider::new("original model");
        provider.batch_calls = calls.clone();
        p.inner.lock().unwrap().stt_provider_cache.insert(
            serde_json::json!(["groq", "whisper-large-v3-turbo", null, false, source]).to_string(),
            Arc::new(provider),
        );
        p.start_recording().unwrap();
        config.managed_inference_access_token = Some("different-session".into());
        p.update_config(config).unwrap();
        let result = p.stop_and_transcribe_detailed().await;
        if source == ModelSource::Byok {
            assert_eq!(result.unwrap().final_text, "original model");
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        } else {
            assert!(
                matches!(result, Err(PipelineError::Config(message)) if message.contains("login session changed"))
            );
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
            assert!(p.clone_last_wav_bytes().is_some());
        }
    }
}

#[tokio::test]
async fn deferred_adapters_keep_the_original_configuration_and_failures_clean_up() {
    for available in [true, false] {
        let mut config = test_config_for_transcription();
        config.llm_config.enabled = false;
        let pipeline =
            SharedPipeline::new_for_tests(config.clone(), Box::new(FakeAudioCapture::new()));
        pipeline.inject_stt_provider_for_tests(
            MOCK_PROVIDER,
            None,
            None,
            Arc::new(MockSttProvider::new("original source")),
        );
        pipeline.start_recording().unwrap();
        config.stt_provider = "different-provider".into();
        pipeline.update_config(config.clone()).unwrap();
        pipeline.inner.lock().unwrap().recording_provider = None;
        if available {
            pipeline.inject_stt_provider_for_tests(
                MOCK_PROVIDER,
                None,
                None,
                Arc::new(MockSttProvider::new("original source")),
            );
        }
        let result = pipeline.stop_and_transcribe_detailed().await;
        if available {
            assert_eq!(result.unwrap().final_text, "original source");
            assert_eq!(pipeline.state(), PipelineState::Idle);
        } else {
            assert!(result.is_err());
            assert_eq!(pipeline.state(), PipelineState::Error);
            assert!(pipeline.clone_last_wav_bytes().is_some());
        }
        assert_eq!(pipeline.config().stt_provider, "different-provider");
    }
}

#[tokio::test]
async fn incomplete_capture_retains_audio_and_never_submits_a_partial_batch() {
    let mut capture = FakeAudioCapture::new();
    capture.incomplete = true;
    let p = SharedPipeline::new_for_tests(test_config_for_transcription(), Box::new(capture));
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut provider = MockSttProvider::new("must not upload");
    provider.batch_calls = calls.clone();
    p.inject_stt_provider_for_tests(MOCK_PROVIDER, None, None, Arc::new(provider));
    p.start_recording().unwrap();
    assert!(matches!(
        p.stop_and_transcribe_detailed().await,
        Err(PipelineError::AudioCapture(_))
    ));
    assert_eq!(p.state(), PipelineState::Error);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(p.clone_last_wav_bytes().is_some());
}

#[cfg(target_os = "linux")]
pub(crate) fn native_model_actions(app: &tauri::AppHandle) {
    use serde_json::json;
    use tauri::Manager;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };
    assert_eq!(
        std::env::var("KOLBOO_NATIVE_WINDOW_TEST").as_deref(),
        Ok("1")
    );
    let window = tauri::WebviewWindowBuilder::new(app, "model-action-test", Default::default())
        .visible(false)
        .build()
        .unwrap();
    let invoke = |command: &str, body: serde_json::Value| {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let webview: &tauri::Webview = window.as_ref();
        webview.clone().on_message(
            tauri::webview::InvokeRequest {
                cmd: command.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: app.invoke_key().into(),
            },
            Box::new(move |_, _, response, _, _| {
                tx.send(response).unwrap();
            }),
        );
        rx.recv_timeout(Duration::from_secs(5)).unwrap()
    };
    tauri::async_runtime::block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .and(wiremock::matchers::body_string_contains(
                "Synthetic failure",
            ))
            .respond_with(ResponseTemplate::new(503))
            .with_priority(1)
            .expect(4)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"message":{"content":" Synthetic output "}})),
            )
            .with_priority(2)
            .expect(8)
            .mount(&server)
            .await;
        let profiles=serde_json::from_value(json!([{"id":"p","name":"Profile",
            "model_selections":{"prompt_tools":{"source":"byok","provider_id":"ollama","model_id":"profile-id"}},
            "presets":[{"id":"q","name":"Preset","model_selections":{"prompt_tools":{"source":"byok","provider_id":"ollama","model_id":"preset-id"}}}]
        }])).unwrap();
        let mut config = test_config_for_transcription();
        config.llm_config.program_prompt_profiles =
            crate::llm::profile_conversion::build_program_prompt_profiles(
                profiles,
                &Default::default(),
            );
        config.llm_config.provider = "ollama".into();
        config.llm_config.model = Some("global-id".into());
        config.llm_config.ollama_url = Some(server.uri());
        // Reuse the updater fixture's managed state. Replacing/removing Tauri
        // state can invalidate borrowed IPC references, so restore only config.
        let pipeline = app.state::<SharedPipeline>();
        let original_config = pipeline.config();
        pipeline.update_config(config).unwrap();
        for with_logs in [false, true] {
            if with_logs {
                assert!(app.manage(crate::request_log::RequestLogStore::new()));
            }
            crate::stats::emit_cost_events_for_current_request_with_duration(
                app,
                crate::stats::EventStatus::Success,
                None,
            );
            let logs = app.try_state::<crate::request_log::RequestLogStore>();
            let recording_id = logs
                .as_ref()
                .map(|logs| logs.start_request("unrelated-recording".into(), None));
            for (command, body) in [
                (
                    "llm_complete",
                    json!({"args":{"provider":"ollama","model":"explicit-id","action":"history_analysis","modelSelection":{"source":"byok","provider_id":"ollama","model_id":"explicit-id"},"systemPrompt":"sys","userPrompt":"input"}}),
                ),
                (
                    "iterate_rewrite_prompt",
                    json!({"profileId":"p","presetId":"q","mode":"new","transcript":"","problemOutput":"Synthetic goal","currentPrompt":"Synthetic prompt"}),
                ),
                (
                    "test_rewrite_with_prompt",
                    json!({"profileId":"p","presetId":"q","transcript":"input","prompt":"sys"}),
                ),
            ] {
                match invoke(command, body) {
                    tauri::ipc::InvokeResponse::Ok(tauri::ipc::InvokeResponseBody::Json(body)) => {
                        let body: serde_json::Value = serde_json::from_str(&body).unwrap();
                        assert_eq!(body["provider_used"], "ollama");
                        assert_eq!(
                            body["model_used"],
                            if command == "llm_complete" {
                                "explicit-id"
                            } else {
                                "preset-id"
                            }
                        );
                    }
                    other => panic!("Model-action IPC failed: {other:?}"),
                }
            }
            let requests = server.received_requests().await.unwrap();
            assert_eq!(requests.len(), if with_logs { 8 } else { 3 });
            let requests = &requests[if with_logs { 5 } else { 0 }..];
            for (index, request) in requests.iter().enumerate() {
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&request.body).unwrap()["model"],
                    if index == 0 {
                        "explicit-id"
                    } else {
                        "preset-id"
                    }
                );
            }
            for args in [
                json!({"provider":"ollama","action":"transcription","systemPrompt":"sys","userPrompt":"input"}),
                json!({"provider":"ollama","profileId":"missing","systemPrompt":"sys","userPrompt":"input"}),
            ] {
                assert!(matches!(
                    invoke("llm_complete", json!({"args":args})),
                    tauri::ipc::InvokeResponse::Err(_)
                ));
            }
            let request_count = server.received_requests().await.unwrap().len();
            for command in ["iterate_rewrite_prompt", "test_rewrite_with_prompt"] {
                assert!(matches!(
                    invoke(
                        command,
                        json!({"profileId":"p","presetId":"missing","mode":"new","transcript":"input","problemOutput":"goal","currentPrompt":"prompt","prompt":"sys"})
                    ),
                    tauri::ipc::InvokeResponse::Err(_)
                ));
            }
            assert_eq!(
                server.received_requests().await.unwrap().len(),
                request_count
            );
            for command in ["iterate_rewrite_prompt", "test_rewrite_with_prompt"] {
                assert!(matches!(
                    invoke(
                        command,
                        json!({"profileId":"p","presetId":"q","mode":"new","transcript":"Synthetic failure","problemOutput":"goal","currentPrompt":"prompt","prompt":"sys"})
                    ),
                    tauri::ipc::InvokeResponse::Err(_)
                ));
            }
            assert_eq!(
                server.received_requests().await.unwrap().len(),
                request_count + 2
            );
            if let Some(logs) = logs.as_ref() {
                assert_eq!(
                    logs.with_current(|log| (log.id.clone(), log.status.clone())),
                    Some((
                        recording_id.clone().unwrap(),
                        crate::request_log::RequestStatus::InProgress
                    ))
                );
                assert_eq!(
                    logs.get_logs(None)
                        .iter()
                        .filter(|log| log.stt_provider.starts_with("prompt-"))
                        .count(),
                    4
                );
                let count = logs.count();
                assert!(matches!(
                    invoke(
                        "iterate_rewrite_prompt",
                        json!({"profileId":"p","presetId":"q","mode":"fixed","transcript":"input","problemOutput":"output","currentPrompt":"prompt"})
                    ),
                    tauri::ipc::InvokeResponse::Err(_)
                ));
                assert_eq!(
                    logs.count(),
                    count,
                    "invalid inputs do not start or cancel a log"
                );
                assert_eq!(logs.with_current(|log| log.id.clone()), recording_id);
                logs.with_current(|log| log.complete_cancelled());
                logs.complete_current();
                logs.clear();
            }
        }
        // The runtime Quick Ask and Quick Replace paths must carry the same
        // atomic choice into the actual adapter, not just into a UI test call.
        use crate::sessions::{
            quick_action_execution as execution, quick_action_lifecycle as lifecycle,
        };
        let selected = crate::model_selection::ModelSelection {
            source: crate::model_selection::ModelSource::Byok,
            provider_id: "ollama".into(),
            model_id: "quick-id".into(),
        };
        let result = TranscriptionResult {
            stt_language_hint: None,
            stt_detected_languages: Vec::new(),
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
        let ask = lifecycle::QuickAskProfileConfig {
            selection: Some(selected.clone()),
            ..Default::default()
        };
        let ocr_config = pipeline.config().ocr_config;
        execution::answer_quick_ask(execution::QuickAskExecution {
            app,
            pipeline: pipeline.inner(),
            request_id: None,
            result: &result,
            fallback_text: "",
            profile_config: &ask,
            probe_plan: lifecycle::QuickActionProbePlan::new(
                lifecycle::QuickActionKind::QuickAsk,
                0,
                crate::commands::text::ContextGrabMethod::None,
                0,
            ),
            ocr_mode: "off",
            ocr_config: &ocr_config,
        })
        .await;
        if let Some(window) = app.get_webview_window("quick-ask") {
            window.close().unwrap();
        }
        let replace = lifecycle::QuickReplaceConfig {
            selection: Some(selected),
            enabled: true,
            provider: Some("ollama".into()),
            model: Some("quick-id".into()),
            system_prompt: "Synthetic instruction".into(),
            include_clipboard_context: false,
        };
        {
            let state = app.state::<crate::state::AppState>();
            let mut probe = state.quick_replace_probe.lock().unwrap();
            probe.epoch = 1;
            probe.ready = true;
            probe.selection_text = Some("Synthetic selection".into());
        }
        let replaced = execution::try_quick_replace(execution::QuickReplaceExecution {
            app,
            pipeline: pipeline.inner(),
            request_id: None,
            config: &replace,
            probe_plan: lifecycle::QuickActionProbePlan::new(
                lifecycle::QuickActionKind::QuickReplace,
                1,
                crate::commands::text::ContextGrabMethod::CtrlC,
                0,
            ),
            ocr_mode: "off",
            ocr_config: &ocr_config,
            output_value: "Synthetic instruction".into(),
        })
        .await;
        assert_eq!(replaced.output_value, "Synthetic output");
        assert!(replaced.failure.is_none());
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 12);
        for request in &requests[10..] {
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&request.body).unwrap()["model"],
                "quick-id"
            );
        }
        pipeline.update_config(original_config).unwrap();
        quick_ask::native_answers(app).await;
    });
    window.close().unwrap();
}

#[test]
fn atomic_stt_routing_preserves_source_inheritance_cache_and_fail_closed_errors() {
    use crate::model_selection::{ModelAction, ModelSelection, ModelSource};
    let choice = |source, provider: &str, model: &str| ModelSelection {
        source,
        provider_id: provider.into(),
        model_id: model.into(),
    };
    let byok = choice(ModelSource::Byok, "groq", "whisper-large-v3-turbo");
    let mut cfg = test_config_for_transcription();
    cfg.managed_inference_enabled = true;
    cfg.managed_stt_preferred = true;
    cfg.managed_inference_gateway_url = Some("https://gateway.example.test".into());
    cfg.managed_inference_access_token = Some("synthetic-session".into());
    cfg.stt_api_keys
        .insert("groq".into(), "synthetic-vendor-key".into());
    cfg.model_selections
        .insert(ModelAction::Transcription, byok.clone());
    let p = SharedPipeline::new_for_tests(cfg, Box::new(FakeAudioCapture::new()));
    let mut inner = p.inner.lock().unwrap();
    let first = stt_provider_resolver::get_or_create_selected_stt_provider(&mut inner, &byok, None)
        .unwrap();
    let again = stt_provider_resolver::get_or_create_selected_stt_provider(&mut inner, &byok, None)
        .unwrap();
    assert!(Arc::ptr_eq(&first, &again));
    let managed = choice(ModelSource::Managed, "groq", "whisper-large-v3-turbo");
    let paid =
        stt_provider_resolver::get_or_create_selected_stt_provider(&mut inner, &managed, None)
            .unwrap();
    assert!(!Arc::ptr_eq(&first, &paid));
    assert_eq!(inner.stt_provider_cache.len(), 2);
    assert!(stt_provider_resolver::get_or_create_selected_stt_provider(
        &mut inner,
        &choice(ModelSource::Managed, "groq", "not-allowed"),
        None
    )
    .is_err());
    assert!(stt_provider_resolver::get_or_create_selected_stt_provider(
        &mut inner,
        &choice(ModelSource::Byok, "groq", ""),
        None
    )
    .is_err());
    inner.config.model_selection_error = Some("Invalid model settings".into());
    assert!(
        stt_provider_resolver::get_or_create_selected_stt_provider(&mut inner, &byok, None)
            .is_err()
    );
    let request = || SttProviderResolutionRequest {
        active_profile: None,
        active_preset: None,
        forced_provider: None,
        forced_model: None,
        forced_language: None,
    };
    assert!(
        stt_provider_resolver::resolve_stt_provider_for_transcription(&mut inner, request())
            .is_err()
    );
    inner.config.model_selection_error = None;
    let mut profile = create_routing_test_profile(Vec::new());
    profile
        .model_selections
        .insert(ModelAction::Transcription, byok.clone());
    let resolved = stt_provider_resolver::resolve_stt_provider_for_transcription(
        &mut inner,
        SttProviderResolutionRequest {
            active_profile: Some(&profile),
            ..request()
        },
    )
    .unwrap();
    assert_eq!(resolved.provider_id, "groq");
    assert_eq!(resolved.model.as_deref(), Some("whisper-large-v3-turbo"));
    inner.config.managed_inference_enabled = false;
    profile
        .model_selections
        .insert(ModelAction::Transcription, managed);
    assert!(
        stt_provider_resolver::resolve_stt_provider_for_transcription(
            &mut inner,
            SttProviderResolutionRequest {
                active_profile: Some(&profile),
                ..request()
            }
        )
        .is_err()
    );
    assert_eq!(
        profile.model_selections[&ModelAction::Transcription].source,
        ModelSource::Managed
    );
    assert_eq!(inner.stt_provider_cache.len(), 2);
}

#[test]
fn unavailable_provider_key_errors_do_not_claim_the_saved_key_is_missing() {
    let config = PipelineConfig::default();
    let stt_result = stt_provider::create_cloud_stt_provider(
        reqwest::Client::new(),
        stt_provider::SttProviderParams {
            provider_id: "groq".into(),
            model: None,
            language: None,
            api_key: String::new(),
            proxy_settings: Default::default(),
            managed_gateway_url: None,
            transcription_prompt: None,
            request_log_store: None,
        },
    );
    let Err(PipelineError::Config(stt_message)) = stt_result else {
        panic!("unavailable key must fail")
    };
    assert_eq!(stt_message,"Could not load a usable API key for STT provider 'groq'. Check provider settings and your system wallet");
    let llm_result = llm_provider::resolve_cached_llm_provider_config(
        &config,
        "groq",
        LlmProviderParams {
            selection: None,
            model: None,
            timeout: Duration::from_secs(1),
            ollama_url: None,
            openai_reasoning_effort: None,
            gemini_thinking_budget: None,
            gemini_thinking_level: None,
            anthropic_thinking_budget: None,
        },
    );
    let Err(PipelineError::Config(llm_message)) = llm_result else {
        panic!("unavailable key must fail")
    };
    assert_eq!(llm_message,"Could not load a usable API key for LLM provider 'groq'. Check provider settings and your system wallet");
}

#[test]
fn failed_locks_and_stale_callbacks_fail_closed_without_creating_providers() {
    use super::transcription_flow::TranscriptionCallbacks;
    let pipeline = SharedPipeline::new_for_tests(
        test_config_for_transcription(),
        Box::new(FakeAudioCapture::new()),
    );
    let cancel = CancellationToken::new();
    let callbacks = PipelineCallbacks {
        inner: pipeline.inner.clone(),
        cancel: cancel.clone(),
    };
    cancel.cancel();
    let params = LlmProviderParams {
        selection: None,
        model: None,
        timeout: Duration::from_secs(1),
        ollama_url: None,
        openai_reasoning_effort: None,
        gemini_thinking_budget: None,
        gemini_thinking_level: None,
        anthropic_thinking_budget: None,
    };
    assert!(matches!(
        callbacks.get_or_create_llm_provider("unconfigured", params.clone()),
        Err(PipelineError::Cancelled)
    ));
    assert!(pipeline.inner.lock().unwrap().llm_provider_cache.is_empty());
    let inner = pipeline.inner.clone();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = inner.lock().unwrap();
        panic!("synthetic lock failure");
    }));
    assert!(matches!(
        pipeline.mark_stt_complete(&CancellationToken::new(), "test"),
        Err(PipelineError::Lock(_))
    ));
    assert!(matches!(
        pipeline.take_session_preset_lock(&CancellationToken::new()),
        Err(PipelineError::Lock(_))
    ));
    assert!(matches!(
        callbacks.get_or_create_llm_provider("unconfigured", params),
        Err(PipelineError::Lock(_))
    ));
}

#[tokio::test]
async fn each_capture_and_saved_retry_cancels_the_previous_session_token() {
    let mut config = test_config_for_transcription();
    config.llm_config.enabled = false;
    let pipeline = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    pipeline.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(MockSttProvider::new("saved result")),
    );
    let previous = CancellationToken::new();
    pipeline.inner.lock().unwrap().cancel_token = Some(previous.clone());
    pipeline.start_recording().unwrap();
    assert!(previous.is_cancelled());
    let capture = pipeline.get_cancel_token().unwrap();
    assert!(!capture.is_cancelled());
    pipeline.inner.lock().unwrap().state = PipelineState::Idle;
    let result = pipeline
        .transcribe_wav_bytes_detailed_for_profile(vec![1, 2, 3], None)
        .await
        .unwrap();
    assert_eq!(result.final_text, "saved result");
    assert!(capture.is_cancelled());
    assert_eq!(pipeline.state(), PipelineState::Idle);
}

/// A fake audio capture backend that returns canned WAV data without using CPAL.
pub(super) struct FakeAudioCapture {
    diagnostics_override: Option<AudioCaptureDiagnostics>,
    level_meter: SharedAudioLevelMeter,
    waveform_meter: SharedAudioWaveformMeter,
    vad_enabled: bool,
    vad_auto_stop: bool,
    wav: Vec<u8>,
    before_wav: Vec<u8>,
    after_wav: Vec<u8>,
    _queued_events: std::collections::VecDeque<AudioCaptureEvent>,
    recovery_path: Option<std::path::PathBuf>,
    cancellation_calls: Arc<std::sync::Mutex<Vec<&'static str>>>,
    fail_recovery_finish: bool,
    incomplete: bool,
}

impl FakeAudioCapture {
    pub fn new() -> Self {
        Self {
            diagnostics_override: None,
            level_meter: SharedAudioLevelMeter::new_for_tests(),
            waveform_meter: SharedAudioWaveformMeter::new_for_tests(),
            vad_enabled: false,
            vad_auto_stop: false,
            wav: vec![1, 2, 3],
            before_wav: vec![9],
            after_wav: vec![8],
            _queued_events: std::collections::VecDeque::new(),
            recovery_path: None,
            cancellation_calls: Arc::new(std::sync::Mutex::new(Vec::new())),
            fail_recovery_finish: false,
            incomplete: false,
        }
    }

    fn diagnostics() -> AudioCaptureDiagnostics {
        AudioCaptureDiagnostics {
            stats: AudioLevelStats {
                duration_secs: 0.5,
                rms: 0.1,
                peak: 0.2,
            },
            speech_detected: None,
        }
    }
}

/// A fake audio capture backend that fails immediately when recording starts.
///
/// This gives us a deterministic regression seam for "no input device" style failures
/// without requiring real hardware setup on the test machine.
struct FailingStartAudioCapture {
    level_meter: SharedAudioLevelMeter,
    waveform_meter: SharedAudioWaveformMeter,
}

impl FailingStartAudioCapture {
    fn new() -> Self {
        Self {
            level_meter: SharedAudioLevelMeter::new_for_tests(),
            waveform_meter: SharedAudioWaveformMeter::new_for_tests(),
        }
    }
}

impl AudioCaptureBackend for FailingStartAudioCapture {
    fn shared_level_meter(&self) -> SharedAudioLevelMeter {
        self.level_meter.clone()
    }

    fn shared_waveform_meter(&self) -> SharedAudioWaveformMeter {
        self.waveform_meter.clone()
    }

    fn level_snapshot(&self) -> AudioLevelSnapshot {
        self.level_meter.snapshot()
    }

    fn set_vad_config(&mut self, _config: crate::audio_capture::VadAutoStopConfig) {}

    fn set_capture_behavior(
        &mut self,
        _hot_mic_enabled: bool,
        _hot_mic_pre_roll_ms: u32,
        _mic_auto_recover_enabled: bool,
        _input_device_name: Option<&str>,
    ) -> Result<(), AudioCaptureError> {
        Ok(())
    }

    fn start_recording_session(
        &mut self,
        _max_duration_secs: f32,
        _input_device_name: Option<&str>,
    ) -> Result<(), AudioCaptureError> {
        Err(AudioCaptureError::NoInputDevice)
    }

    fn stop_and_get_wav_with_diagnostics(
        &mut self,
        _cfg: AudioEncodeConfig,
    ) -> Result<(Vec<u8>, AudioCaptureDiagnostics), AudioCaptureError> {
        Err(AudioCaptureError::NoInputDevice)
    }

    fn stop_and_get_wav_before_after(
        &mut self,
        _after_cfg: AudioEncodeConfig,
    ) -> Result<(Vec<u8>, Vec<u8>, AudioCaptureDiagnostics), AudioCaptureError> {
        Err(AudioCaptureError::NoInputDevice)
    }

    fn stop_recording(&mut self) {}
    fn stop(&mut self) {}

    fn poll_vad_event(&self) -> Option<AudioCaptureEvent> {
        None
    }

    fn is_vad_auto_stop_enabled(&self) -> bool {
        false
    }

    fn set_live_audio_tx(&mut self, _tx: Option<crate::stt::streaming::StreamingAudioSender>) {
        // No-op for tests.
    }

    fn capture_sample_rate(&self) -> u32 {
        16000
    }
}

impl AudioCaptureBackend for FakeAudioCapture {
    fn recording_progress(&self) -> (f64, bool) {
        (0.5, self.incomplete)
    }
    fn recovery_path(&self) -> Option<std::path::PathBuf> {
        self.recovery_path.clone()
    }

    fn set_recovery_path(
        &mut self,
        path: Option<std::path::PathBuf>,
    ) -> Result<(), AudioCaptureError> {
        self.recovery_path = path;
        Ok(())
    }

    fn finish_recovery(&mut self) -> Result<(), AudioCaptureError> {
        self.cancellation_calls.lock().unwrap().push("finish");
        if self.fail_recovery_finish {
            return Err(AudioCaptureError::Encoding("Synthetic sync failure".into()));
        }
        Ok(())
    }

    fn discard_recovery(&mut self) -> Result<(), AudioCaptureError> {
        self.cancellation_calls.lock().unwrap().push("discard");
        if let Some(path) = self.recovery_path.take() {
            crate::audio_capture::journal::discard(&path).unwrap();
        }
        Ok(())
    }

    fn set_paused(&mut self, _paused: bool) -> Result<(), AudioCaptureError> {
        Ok(())
    }
    fn shared_level_meter(&self) -> SharedAudioLevelMeter {
        self.level_meter.clone()
    }

    fn shared_waveform_meter(&self) -> SharedAudioWaveformMeter {
        self.waveform_meter.clone()
    }

    fn level_snapshot(&self) -> AudioLevelSnapshot {
        self.level_meter.snapshot()
    }

    fn set_vad_config(&mut self, config: crate::audio_capture::VadAutoStopConfig) {
        self.vad_enabled = config.enabled;
        self.vad_auto_stop = config.auto_stop;
    }

    fn set_capture_behavior(
        &mut self,
        _hot_mic_enabled: bool,
        _hot_mic_pre_roll_ms: u32,
        _mic_auto_recover_enabled: bool,
        _input_device_name: Option<&str>,
    ) -> Result<(), AudioCaptureError> {
        Ok(())
    }

    fn start_recording_session(
        &mut self,
        _max_duration_secs: f32,
        _input_device_name: Option<&str>,
    ) -> Result<(), AudioCaptureError> {
        Ok(())
    }

    fn stop_and_get_wav_with_diagnostics(
        &mut self,
        _cfg: AudioEncodeConfig,
    ) -> Result<(Vec<u8>, AudioCaptureDiagnostics), AudioCaptureError> {
        Ok((
            self.wav.clone(),
            self.diagnostics_override
                .clone()
                .unwrap_or_else(Self::diagnostics),
        ))
    }

    fn stop_and_get_wav_before_after(
        &mut self,
        _after_cfg: AudioEncodeConfig,
    ) -> Result<(Vec<u8>, Vec<u8>, AudioCaptureDiagnostics), AudioCaptureError> {
        Ok((
            self.before_wav.clone(),
            self.after_wav.clone(),
            Self::diagnostics(),
        ))
    }

    fn stop_recording(&mut self) {
        self.cancellation_calls.lock().unwrap().push("stop");
    }
    fn stop(&mut self) {}

    fn poll_vad_event(&self) -> Option<AudioCaptureEvent> {
        None
    }

    fn is_vad_auto_stop_enabled(&self) -> bool {
        self.vad_enabled && self.vad_auto_stop
    }

    fn set_live_audio_tx(&mut self, _tx: Option<crate::stt::streaming::StreamingAudioSender>) {
        // No-op for tests.
    }

    fn capture_sample_rate(&self) -> u32 {
        16000
    }
}

/// Configurable behavior for mock providers.
#[derive(Clone, Default)]
pub(super) struct MockBehavior {
    /// Artificial delay before returning a response.
    pub delay: Option<std::time::Duration>,
    /// If set, the mock will return this error instead of success.
    pub error: Option<String>,
}

/// A mock STT provider that returns a canned transcript without making network calls.
/// Supports configurable latency and error simulation for testing edge cases.
struct MockSttProvider {
    text: String,
    behavior: MockBehavior,
    realtime_only: bool,
    batch_calls: Arc<std::sync::atomic::AtomicUsize>,
}

impl MockSttProvider {
    fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            behavior: MockBehavior::default(),
            realtime_only: false,
            batch_calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    #[allow(dead_code)]
    fn with_behavior(mut self, behavior: MockBehavior) -> Self {
        self.behavior = behavior;
        self
    }
}

#[async_trait]
impl SttProvider for MockSttProvider {
    fn requires_streaming(&self) -> bool {
        self.realtime_only
    }
    async fn transcribe(
        &self,
        _audio: &[u8],
        _format: &AudioFormat,
    ) -> Result<String, crate::stt::SttError> {
        self.batch_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Some(delay) = self.behavior.delay {
            tokio::time::sleep(delay).await;
        }
        if let Some(ref err) = self.behavior.error {
            return Err(crate::stt::SttError::Api(err.clone()));
        }
        Ok(self.text.clone())
    }

    fn name(&self) -> &'static str {
        "mock"
    }
}

/// A mock LLM provider that returns a canned completion without making network calls.
/// Supports configurable latency and error simulation for testing edge cases.
struct MockLlmProvider {
    response: String,
    behavior: MockBehavior,
}

impl MockLlmProvider {
    fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
            behavior: MockBehavior::default(),
        }
    }

    #[allow(dead_code)]
    fn with_behavior(mut self, behavior: MockBehavior) -> Self {
        self.behavior = behavior;
        self
    }
}

/// A mock LLM provider that returns different outputs depending on whether OCR context
/// is present in the user message.
struct ConditionalOcrLlmProvider {
    with_ocr: String,
    without_ocr: String,
}

impl ConditionalOcrLlmProvider {
    fn new(with_ocr: impl Into<String>, without_ocr: impl Into<String>) -> Self {
        Self {
            with_ocr: with_ocr.into(),
            without_ocr: without_ocr.into(),
        }
    }
}

#[async_trait]
impl crate::llm::LlmProvider for MockLlmProvider {
    async fn complete_streaming(
        &self,
        system_prompt: &str,
        user_message: &str,
        _notify: crate::llm::streaming::Delta<'_>,
    ) -> Result<String, crate::llm::LlmError> {
        self.complete(system_prompt, user_message).await
    }
    async fn complete(
        &self,
        _system_prompt: &str,
        _user_message: &str,
    ) -> Result<String, crate::llm::LlmError> {
        if let Some(delay) = self.behavior.delay {
            tokio::time::sleep(delay).await;
        }
        if let Some(ref err) = self.behavior.error {
            return Err(crate::llm::LlmError::Api(err.clone()));
        }
        Ok(self.response.clone())
    }

    fn name(&self) -> &'static str {
        "mock"
    }

    fn model(&self) -> &str {
        "mock-model"
    }
}

#[async_trait]
impl crate::llm::LlmProvider for ConditionalOcrLlmProvider {
    async fn complete_streaming(
        &self,
        system_prompt: &str,
        user_message: &str,
        _notify: crate::llm::streaming::Delta<'_>,
    ) -> Result<String, crate::llm::LlmError> {
        self.complete(system_prompt, user_message).await
    }
    async fn complete(
        &self,
        _system_prompt: &str,
        user_message: &str,
    ) -> Result<String, crate::llm::LlmError> {
        let has_ocr = user_message.contains("OCR context from the currently active window:");
        Ok(if has_ocr {
            self.with_ocr.clone()
        } else {
            self.without_ocr.clone()
        })
    }

    fn name(&self) -> &'static str {
        "mock"
    }

    fn model(&self) -> &str {
        "mock-model"
    }
}

fn set_state_for_test(
    pipeline: &SharedPipeline,
    state: PipelineState,
    token: Option<CancellationToken>,
) {
    let mut inner = pipeline.inner.lock().expect("pipeline lock");
    inner.state = state;
    inner.cancel_token = token;
}

fn test_config_with_max_recording_bytes() -> PipelineConfig {
    PipelineConfig {
        max_recording_bytes: 1024,
        ..Default::default()
    }
}

fn test_config_for_transcription() -> PipelineConfig {
    let mut config = test_config_with_max_recording_bytes();
    config.stt_provider = MOCK_PROVIDER.to_string();
    config.stt_language = None;
    config.quiet_audio_gate_enabled = false;
    config
}

fn mock_llm_config(
    enabled: bool,
    program_prompt_profiles: Vec<crate::llm::ProgramPromptProfile>,
) -> crate::llm::LlmConfig {
    crate::llm::LlmConfig {
        model_selections: Default::default(),
        selection: None,
        selection_error: None,
        managed_access_token: None,
        enabled,
        custom_providers: Vec::new(),
        provider: MOCK_PROVIDER.to_string(),
        api_key: String::new(),
        model: None,
        ollama_url: None,
        managed_gateway_url: None,
        openai_reasoning_effort: None,
        gemini_thinking_budget: None,
        gemini_thinking_level: None,
        anthropic_thinking_budget: None,
        prompts: crate::llm::PromptSections::default(),
        program_prompt_profiles,
        timeout: std::time::Duration::from_secs(30),
    }
}

fn insert_mock_llm_api_key(config: &mut PipelineConfig) {
    config
        .llm_api_keys
        .insert(MOCK_PROVIDER.to_string(), MOCK_API_KEY.to_string());
}

#[test]
fn test_shared_pipeline_creation() {
    let config = PipelineConfig {
        stt_api_key: "test-key".to_string(),
        ..Default::default()
    };
    let pipeline = SharedPipeline::new(config);
    assert_eq!(pipeline.state(), PipelineState::Idle);
    assert!(!pipeline.is_error());
}

#[tokio::test]
async fn deferred_provider_resolution_uses_frozen_settings_and_restores_current_settings() {
    let mut config = test_config_for_transcription();
    config.stt_model = Some("captured".into());
    config.stt_language = Some("yi".into());
    config.llm_config.enabled = false;
    let pipeline = SharedPipeline::new_for_tests(config.clone(), Box::new(FakeAudioCapture::new()));
    let provider = Arc::new(MockSttProvider::new("captured provider result"));
    pipeline.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        Some("captured"),
        Some("yi"),
        provider.clone(),
    );
    pipeline.start_recording().unwrap();
    config.stt_model = Some("later".into());
    config.stt_language = Some("he".into());
    pipeline.update_config(config).unwrap();
    // Deferred native/local initialization has no constructed provider or
    // request context yet. Its captured configuration still owns resolution.
    {
        let mut inner = pipeline.inner.lock().unwrap();
        inner.recording_provider = None;
        inner.recording_profile_context = None;
    }
    pipeline.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        Some("captured"),
        Some("yi"),
        provider.clone(),
    );
    assert_eq!(
        pipeline
            .stop_and_transcribe_detailed()
            .await
            .unwrap()
            .stt_text,
        "captured provider result"
    );
    assert_eq!(
        provider
            .batch_calls
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert_eq!(pipeline.config().stt_model.as_deref(), Some("later"));
    assert_eq!(pipeline.config().stt_language.as_deref(), Some("he"));
}

#[tokio::test]
async fn recording_keeps_its_original_model_hint_and_provider_across_config_updates() {
    let mut config = test_config_for_transcription();
    config.stt_model = Some("first".into());
    config.stt_language = Some("yi".into());
    config.llm_config.enabled = false;
    let pipeline = SharedPipeline::new_for_tests(config.clone(), Box::new(FakeAudioCapture::new()));
    let first = Arc::new(MockSttProvider::new("first result"));
    let first_calls = first.batch_calls.clone();
    pipeline.inject_stt_provider_for_tests(MOCK_PROVIDER, Some("first"), Some("yi"), first);
    pipeline.start_recording().unwrap();
    config.stt_model = Some("second".into());
    config.stt_language = Some("he".into());
    config.stt_input_mode = crate::settings::SttInputMode::AfterRecording;
    pipeline.update_config(config).unwrap();
    let second = Arc::new(MockSttProvider::new("second result"));
    let second_calls = second.batch_calls.clone();
    pipeline.inject_stt_provider_for_tests(MOCK_PROVIDER, Some("second"), Some("he"), second);
    assert_eq!(
        pipeline
            .stop_and_transcribe_detailed()
            .await
            .unwrap()
            .stt_text,
        "first result"
    );
    assert_eq!(first_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(second_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(pipeline.inner.lock().unwrap().recording_config.is_none());
    pipeline.start_recording().unwrap();
    assert_eq!(
        pipeline
            .stop_and_transcribe_detailed()
            .await
            .unwrap()
            .stt_text,
        "second result"
    );
    assert_eq!(second_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn incompatible_realtime_only_input_mode_retains_audio_without_submitting() {
    let mut config = test_config_for_transcription();
    config.stt_input_mode = crate::settings::SttInputMode::AfterRecording;
    let pipeline = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    let mut provider = MockSttProvider::new("must not upload");
    provider.realtime_only = true;
    let calls = provider.batch_calls.clone();
    pipeline.inject_stt_provider_for_tests(MOCK_PROVIDER, None, None, Arc::new(provider));
    pipeline.start_recording().unwrap();
    assert!(matches!(
        pipeline.stop_and_transcribe_detailed().await,
        Err(PipelineError::Config(_))
    ));
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(pipeline.clone_last_wav_bytes().is_some());
    assert_eq!(pipeline.state(), PipelineState::Error);
}

#[tokio::test]
async fn streaming_finalize_handles_success_fallback_and_realtime_failure() {
    for (success, realtime, live_started) in [
        (true, false, false),
        (true, false, true),
        (false, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        let mut config = test_config_for_transcription();
        config.llm_config.enabled = false;
        let pipeline = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
        let mut provider = MockSttProvider::new("batch fallback");
        provider.realtime_only = realtime;
        let batch_calls = provider.batch_calls.clone();
        pipeline.inject_stt_provider_for_tests(MOCK_PROVIDER, None, None, Arc::new(provider));
        pipeline.start_recording().unwrap();
        pipeline
            .inner
            .lock()
            .unwrap()
            .live_output_active
            .store(live_started, std::sync::atomic::Ordering::SeqCst);
        let (audio_tx, _audio_rx) = tokio::sync::mpsc::channel(1);
        let (_partial_tx, partial_rx) = tokio::sync::mpsc::channel(1);
        let task = tokio::spawn(async move {
            if success {
                Ok("  streaming result  ".into())
            } else {
                Err(crate::stt::SttError::Api("synthetic failure".into()))
            }
        });
        pipeline.inner.lock().unwrap().active_streaming_session = Some(
            crate::stt::streaming::StreamingSttSession::new(audio_tx, partial_rx, task),
        );
        let result = pipeline.stop_and_transcribe_detailed().await;
        if !success && (realtime || live_started) {
            assert!(matches!(result, Err(PipelineError::Stt(_))));
            assert_eq!(pipeline.state(), PipelineState::Error);
            assert!(pipeline.inner.lock().unwrap().last_wav_bytes.is_some());
        } else {
            let result = result.unwrap();
            assert_eq!(result.live_output_completed, live_started);
            assert_eq!(
                result.final_text,
                if success {
                    // Dictation trims the leading boundary, preserving the
                    // provider's trailing whitespace for insertion.
                    "streaming result  "
                } else {
                    "batch fallback"
                }
            );
            assert_eq!(pipeline.state(), PipelineState::Idle);
        }
        assert_eq!(
            batch_calls.load(std::sync::atomic::Ordering::SeqCst),
            usize::from(!success && !realtime && !live_started)
        );
        assert!(pipeline
            .inner
            .lock()
            .unwrap()
            .active_streaming_session
            .is_none());
    }
}

#[tokio::test]
async fn failed_stream_with_a_poisoned_state_lock_cannot_upload_again() {
    let mut config = test_config_for_transcription();
    config.llm_config.enabled = false;
    let pipeline = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    let provider = MockSttProvider::new("must not upload again");
    let batch_calls = provider.batch_calls.clone();
    pipeline.inject_stt_provider_for_tests(MOCK_PROVIDER, None, None, Arc::new(provider));
    pipeline.start_recording().unwrap();
    let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel(1);
    let (_partial_tx, partial_rx) = tokio::sync::mpsc::channel(1);
    let inner = pipeline.inner.clone();
    let task = tokio::spawn(async move {
        while audio_rx.recv().await.is_some() {}
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = inner.lock().unwrap();
            panic!("synthetic state-lock failure");
        }));
        Err(crate::stt::SttError::Api("synthetic stream failure".into()))
    });
    pipeline.inner.lock().unwrap().active_streaming_session = Some(
        crate::stt::streaming::StreamingSttSession::new(audio_tx, partial_rx, task),
    );
    assert!(matches!(
        pipeline.stop_and_transcribe_detailed().await,
        Err(PipelineError::Lock(_))
    ));
    assert_eq!(batch_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[tokio::test]
async fn streaming_handoff_owns_or_aborts_both_tasks() {
    for outcome in ["recording", "stopped", "cancelled", "error", "poisoned"] {
        let p = SharedPipeline::new_for_tests(
            test_config_for_transcription(),
            Box::new(FakeAudioCapture::new()),
        );
        p.start_recording().unwrap();
        let cancel = p.get_cancel_token().unwrap();
        let epoch = p.session_epoch().unwrap();
        let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel(1);
        let (_partial_tx, partial_rx) = tokio::sync::mpsc::channel(1);
        let (provider_alive, mut provider_dropped) = tokio::sync::oneshot::channel::<()>();
        let (consumer_alive, mut consumer_dropped) = tokio::sync::oneshot::channel::<()>();
        let (ready, started) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _alive = provider_alive;
            ready.send(()).unwrap();
            while audio_rx.recv().await.is_some() {}
            Ok("owned result".into())
        });
        started.await.unwrap();
        let mut session = StreamingSttSession::new(audio_tx, partial_rx, task);
        let (ready, started) = tokio::sync::oneshot::channel();
        session.defer_partial_consumer(async move {
            let _alive = consumer_alive;
            ready.send(()).unwrap();
            std::future::pending::<()>().await;
        });
        match outcome {
            "stopped" => p.inner.lock().unwrap().state = PipelineState::Transcribing,
            "error" => p.inner.lock().unwrap().state = PipelineState::Error,
            "cancelled" => cancel.cancel(),
            "poisoned" => {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _guard = p.inner.lock().unwrap();
                    panic!("synthetic lock poison");
                }));
            }
            _ => {}
        }
        p.install_concurrent_streaming_session(&cancel, epoch, session);
        if outcome == "recording" || outcome == "stopped" {
            started.await.unwrap();
            assert!(p.inner.lock().unwrap().active_streaming_session.is_some());
            assert!(matches!(
                provider_dropped.try_recv(),
                Err(tokio::sync::oneshot::error::TryRecvError::Empty)
            ));
            assert!(matches!(
                consumer_dropped.try_recv(),
                Err(tokio::sync::oneshot::error::TryRecvError::Empty)
            ));
            p.cancel();
        } else {
            assert!(
                started.await.is_err(),
                "rejected handoff must never begin consumer work"
            );
            if outcome != "poisoned" {
                assert!(p.inner.lock().unwrap().active_streaming_session.is_none());
            }
        }
        assert!(provider_dropped.await.is_err());
        assert!(consumer_dropped.await.is_err());
    }
}

#[tokio::test]
async fn state_lock_failure_after_connection_completion_cannot_trigger_a_batch_upload() {
    let mut config = test_config_for_transcription();
    config.llm_config.enabled = false;
    let pipeline = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    let provider = MockSttProvider::new("must not upload");
    let batch_calls = provider.batch_calls.clone();
    pipeline.inject_stt_provider_for_tests(MOCK_PROVIDER, None, None, Arc::new(provider));
    pipeline.start_recording().unwrap();
    let inner = pipeline.inner.clone();
    pipeline.inner.lock().unwrap().pending_connection = Some(
        crate::stt::streaming::OwnedStreamingTask(tokio::spawn(async move {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = inner.lock().unwrap();
                panic!("synthetic lock failure during connection completion");
            }));
        })),
    );
    assert!(matches!(
        pipeline.stop_and_transcribe_detailed().await,
        Err(PipelineError::Lock(_))
    ));
    assert_eq!(batch_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
}

/// Exercise the concrete event-forwarding path with real Wry, but synthetic
/// provider/capture tasks. No recording device, external service or insertion.
#[cfg(target_os = "linux")]
pub(crate) fn native_recording_output(app: &tauri::AppHandle) {
    use tauri::Manager;
    use tauri_plugin_store::StoreExt;
    let pipeline = app.state::<SharedPipeline>();
    let original_config = pipeline.config();
    let original_capture = {
        let mut inner = pipeline.inner.lock().unwrap();
        assert_eq!(inner.state, PipelineState::Idle);
        std::mem::replace(&mut inner.audio_capture, Box::new(FakeAudioCapture::new()))
    };
    let store = app.store("settings.json").unwrap();
    let settings = [
        "output_mode",
        "quick_replace_enabled",
        "quick_ask_enabled",
        "clipboard_privacy_mode",
    ];
    let previous: Vec<_> = settings.iter().map(|key| store.get(*key)).collect();
    store.set("output_mode", serde_json::json!("clipboard"));
    for key in &settings[1..] {
        store.set(*key, serde_json::json!(false));
    }
    let mut config = test_config_for_transcription();
    config.llm_config.enabled = false;
    pipeline.update_config(config).unwrap();
    pipeline.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(MockSttProvider::new("Synthetic native output")),
    );
    let state = app.state::<crate::state::AppState>();
    crate::core::recording::start_recording(
        app,
        &state,
        false,
        crate::audio::AudioCue::Kolboo,
        &None,
        crate::core::recording::PlayingAudioHandling::None,
        "native fixture",
    );
    assert_eq!(pipeline.state(), PipelineState::Recording);
    let request_id = app
        .state::<crate::request_log::RequestLogStore>()
        .with_current(|log| log.id.clone())
        .unwrap();
    crate::stop_recording(
        app,
        &state,
        false,
        crate::audio::AudioCue::Kolboo,
        &None,
        crate::core::recording::PlayingAudioHandling::None,
        "native fixture",
    );
    let history = app.state::<crate::history::HistoryStorage>();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(entry) = history.get_by_id(&request_id).unwrap() {
            if entry.status == crate::history::HistoryStatus::Success {
                assert_eq!(entry.text, "Synthetic native output");
                break;
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "native output did not complete its history row"
        );
        std::thread::yield_now();
    }
    assert_eq!(pipeline.state(), PipelineState::Idle);
    assert_eq!(
        arboard::Clipboard::new().unwrap().get_text().unwrap(),
        "Synthetic native output"
    );
    let delivered =
        tauri::async_runtime::block_on(crate::text::inject::output_text_with_app_async(
            app.clone(),
            "Synthetic explicit output".into(),
            crate::text::inject::OutputMode::Clipboard,
            false,
            false,
            crate::text::key_inject::PasteShortcut::CtrlV,
        ))
        .unwrap();
    assert_eq!(
        delivered,
        crate::text::inject::OutputDelivery::RequestedMode
    );
    assert_eq!(
        arboard::Clipboard::new().unwrap().get_text().unwrap(),
        "Synthetic explicit output"
    );
    pipeline.inner.lock().unwrap().audio_capture = original_capture;
    pipeline.update_config(original_config).unwrap();
    for (key, value) in settings.into_iter().zip(previous) {
        if let Some(value) = value {
            store.set(key, value);
        } else {
            store.delete(key);
        }
    }
}

#[cfg(target_os = "linux")]
pub(crate) async fn native_managed_stt_refresh(
    app: &tauri::AppHandle,
    provider: Arc<dyn SttProvider>,
    original_token: String,
) -> Result<String, PipelineError> {
    use tauri::Manager;
    let pipeline = app.state::<SharedPipeline>();
    pipeline.set_app_handle(app.clone());
    pipeline
        .run_batch_stt_request(
            provider,
            "groq",
            Some("whisper-large-v3-turbo".into()),
            Some("he".into()),
            Some(original_token),
            b"synthetic audio",
            &Default::default(),
            std::time::Duration::from_secs(5),
            &CancellationToken::new(),
            "native owned refresh",
            "meeting_upload",
            None,
        )
        .await
        .map(|result| result.text)
}

#[cfg(target_os = "linux")]
pub(crate) fn native_completed_upload_retry(app: &tauri::AppHandle) {
    use crate::recordings::options::{MeetingModel, RecordingMode, RecordingPreferences};
    use crate::stt::{SttTranscript, SttUploadContext};
    use tauri::Manager;
    struct Job(Arc<std::sync::atomic::AtomicUsize>);
    #[async_trait]
    impl SttProvider for Job {
        fn name(&self) -> &'static str {
            "yiddishlabs"
        }
        async fn transcribe(
            &self,
            _: &[u8],
            _: &AudioFormat,
        ) -> Result<String, crate::stt::SttError> {
            panic!("Standard retry must use an owned upload, not dictation");
        }
        async fn transcribe_upload(
            &self,
            audio: &[u8],
            _: &AudioFormat,
            context: &SttUploadContext,
        ) -> Result<SttTranscript, crate::stt::SttError> {
            assert!(audio.starts_with(b"RIFF"));
            assert_eq!(context.purpose, crate::stt::UploadPurpose::Meeting);
            assert!(context.checkpoint_path.parent().unwrap().is_dir());
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(SttTranscript {
                text: "שלום — synthetic retry".into(),
                segments: Vec::new(),
            })
        }
    }
    let pipeline = app.state::<SharedPipeline>();
    let original = pipeline.config();
    let mut config = test_config_for_transcription();
    config.llm_config.enabled = false;
    pipeline.update_config(config).unwrap();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    pipeline.inject_stt_provider_for_tests(
        "yiddishlabs",
        Some(crate::stt::yiddishlabs::STANDARD_MODEL),
        None,
        Arc::new(Job(calls.clone())),
    );
    let store = app.state::<crate::recordings::RecordingStore>();
    let source = format!("{}-final", uuid::Uuid::new_v4());
    let mut buffer = crate::audio_capture::AudioBuffer::new(16000, 1, 1.0);
    buffer.append(&vec![0.25; 16000]);
    store
        .save_wav(&source, &buffer.to_wav_bytes().unwrap())
        .unwrap();
    store
        .save_options(
            &source,
            &RecordingPreferences {
                mode: RecordingMode::Meeting,
                meeting_model: Some(MeetingModel {
                    provider: "yiddishlabs".into(),
                    model: crate::stt::yiddishlabs::STANDARD_MODEL.into(),
                    use_managed: false,
                }),
                language_hint: None,
            },
        )
        .unwrap();
    for _ in 0..2 {
        let result = tauri::async_runtime::block_on(
            crate::commands::recording::pipeline_retry_transcription_impl(
                app.clone(),
                pipeline.inner().clone(),
                source.clone(),
            ),
        )
        .unwrap();
        assert_eq!(result, "שלום — synthetic retry");
        assert_eq!(pipeline.state(), PipelineState::Idle);
        assert!(!pipeline.is_recovering());
    }
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(store.transcription_checkpoint(&source).unwrap().exists());
    assert!(store.delete_wav_if_exists(&source).unwrap());
    pipeline.update_config(original).unwrap();
}

#[cfg(target_os = "linux")]
pub(crate) fn native_streaming_consumer(app: &tauri::AppHandle) {
    use tauri::Listener;
    struct StreamingProvider;
    #[async_trait]
    impl SttProvider for StreamingProvider {
        fn name(&self) -> &'static str {
            "mock"
        }
        fn supports_streaming(&self) -> bool {
            true
        }
        async fn transcribe(
            &self,
            _: &[u8],
            _: &AudioFormat,
        ) -> Result<String, crate::stt::SttError> {
            panic!("native streaming fixture must never use batch")
        }
        async fn start_streaming(
            &self,
            _: u32,
        ) -> Result<StreamingSttSession, crate::stt::SttError> {
            let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel(1);
            let (partial_tx, partial_rx) = tokio::sync::mpsc::channel(1);
            let task = tokio::spawn(async move {
                while audio_rx.recv().await.is_some() {}
                partial_tx
                    .send(crate::stt::streaming::PartialTranscript {
                        text: "א גוטן טאָג".into(),
                        committed_text: None,
                    })
                    .await
                    .unwrap();
                Ok("א גוטן טאָג".into())
            });
            Ok(StreamingSttSession::new(audio_tx, partial_rx, task))
        }
    }
    let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
    let listener = app.listen(crate::events::EVENT_STT_PARTIAL_TRANSCRIPT, move |event| {
        events.send(event.payload().to_string()).unwrap();
    });
    tauri::async_runtime::block_on(async {
        let mut config = test_config_for_transcription();
        config.llm_config.enabled = false;
        for atomic in [false, true] {
            let selected = crate::model_selection::ModelSelection {
                source: crate::model_selection::ModelSource::Byok,
                provider_id: MOCK_PROVIDER.into(),
                model_id: "mock-model".into(),
            };
            if atomic {
                config
                    .model_selections
                    .insert(crate::model_selection::ModelAction::Transcription, selected);
            }
            let p =
                SharedPipeline::new_for_tests(config.clone(), Box::new(FakeAudioCapture::new()));
            if atomic {
                p.inner.lock().unwrap().stt_provider_cache.insert(
                    serde_json::json!([MOCK_PROVIDER, "mock-model", null, false, "byok"])
                        .to_string(),
                    Arc::new(StreamingProvider),
                );
            } else {
                p.inject_stt_provider_for_tests(
                    MOCK_PROVIDER,
                    None,
                    None,
                    Arc::new(StreamingProvider),
                );
            }
            p.start_recording().unwrap();
            if atomic {
                p.start_concurrent_streaming(app);
                p.start_concurrent_streaming(app);
            } else {
                p.try_start_concurrent_streaming(app).await;
                assert!(p.inner.lock().unwrap().active_streaming_session.is_some());
            }
            let result = p.stop_and_transcribe_detailed().await.unwrap();
            assert_eq!(result.final_text, "א גוטן טאָג");
            // Finalization guarantees all queued preview events have been emitted.
            let event = received.try_recv().unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&event).unwrap(),
                serde_json::json!({"kind": "started", "session_epoch": p.session_epoch().unwrap()})
            );
            let event = received.try_recv().unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&event).unwrap(),
                serde_json::json!({"kind": "started", "session_epoch": p.session_epoch().unwrap()})
            );
            let event = received.try_recv().unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&event).unwrap(),
                serde_json::json!({"kind": "partial", "session_epoch": p.session_epoch().unwrap(), "text": "א גוטן טאָג"})
            );
            assert!(received.try_recv().is_err());
            assert!(!result.live_output_completed);
            assert_eq!(p.state(), PipelineState::Idle);
        }
    });
    app.unlisten(listener);
    tauri::async_runtime::block_on(native_streaming_input_policy(app));
}

#[cfg(target_os = "linux")]
async fn native_streaming_input_policy(app: &tauri::AppHandle) {
    use crate::model_selection::{ModelAction, ModelSelection, ModelSource};
    for case in ["idle", "after", "managed", "missing", "stale", "poison"] {
        let mut config = test_config_for_transcription();
        config.llm_config.enabled = false;
        if case == "after" {
            config.stt_input_mode = crate::settings::SttInputMode::AfterRecording;
        }
        if case == "managed" {
            config.stt_simulated_streaming = true;
            config.managed_inference_enabled = true;
            config.managed_inference_gateway_url = Some("http://must-not-be-used.invalid".into());
            config.managed_inference_access_token = Some("synthetic-session".into());
            config.model_selections.insert(
                ModelAction::Transcription,
                ModelSelection {
                    source: ModelSource::Managed,
                    provider_id: "groq".into(),
                    model_id: "whisper-large-v3-turbo".into(),
                },
            );
        }
        let pipeline = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut provider = MockSttProvider::new("must not upload");
        provider.batch_calls = calls.clone();
        if case == "managed" {
            pipeline.inner.lock().unwrap().stt_provider_cache.insert(
                serde_json::json!(["groq", "whisper-large-v3-turbo", null, false, "managed"])
                    .to_string(),
                Arc::new(provider),
            );
        } else {
            pipeline.inject_stt_provider_for_tests(MOCK_PROVIDER, None, None, Arc::new(provider));
        }
        pipeline.start_concurrent_streaming(app); // Idle never starts a connection task.
        assert!(pipeline.inner.lock().unwrap().pending_connection.is_none());
        if case != "idle" {
            pipeline.start_recording().unwrap();
        }
        if case == "missing" {
            pipeline.inner.lock().unwrap().recording_provider = None;
        }
        if case == "poison" {
            let inner = pipeline.inner.clone();
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = inner.lock().unwrap();
                panic!("synthetic pipeline lock failure");
            }));
            pipeline.start_concurrent_streaming(app);
            pipeline.try_start_concurrent_streaming(app).await;
            pipeline.connect_concurrent_streaming(app, 0).await;
        } else {
            if case == "stale" {
                pipeline.connect_concurrent_streaming(app, u64::MAX).await;
            } else {
                pipeline.try_start_concurrent_streaming(app).await;
            }
            let inner = pipeline.inner.lock().unwrap();
            assert!(inner.pending_connection.is_none());
            assert!(inner.active_streaming_session.is_none());
            drop(inner);
            pipeline.force_reset();
        }
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
}

#[cfg(target_os = "linux")]
pub(crate) async fn native_live_output(app: &tauri::AppHandle, focus: &dyn Fn(bool)) {
    use tauri_plugin_store::StoreExt;
    struct Provider;
    #[async_trait]
    impl SttProvider for Provider {
        fn name(&self) -> &'static str {
            "mock"
        }
        fn supports_streaming(&self) -> bool {
            true
        }
        async fn transcribe(
            &self,
            _: &[u8],
            _: &AudioFormat,
        ) -> Result<String, crate::stt::SttError> {
            panic!("live output cannot use batch fallback");
        }
        async fn start_streaming(
            &self,
            _: u32,
        ) -> Result<StreamingSttSession, crate::stt::SttError> {
            let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel(4);
            let (partial_tx, partial_rx) = tokio::sync::mpsc::channel(4);
            let task = tokio::spawn(async move {
                while audio_rx.recv().await.is_some() {}
                partial_tx
                    .send(crate::stt::streaming::PartialTranscript {
                        text: "Synthetic committed text".into(),
                        committed_text: Some("Synthetic committed text".into()),
                    })
                    .await
                    .unwrap();
                Ok("Synthetic committed text".into())
            });
            Ok(StreamingSttSession::new(audio_tx, partial_rx, task))
        }
    }
    let store = app.store("settings.json").unwrap();
    let keys = ["output_mode", "quick_replace_enabled"];
    let old: Vec<_> = keys.iter().map(|key| store.get(*key)).collect();
    store.set(keys[0], serde_json::json!("clipboard"));
    store.set(keys[1], serde_json::json!(false));
    for mode in 0..3 {
        focus(mode != 1);
        let mut config = test_config_for_transcription();
        config.llm_config.enabled = false;
        config.stt_live_output = true;
        let pipeline = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
        pipeline.inject_stt_provider_for_tests(MOCK_PROVIDER, None, None, Arc::new(Provider));
        pipeline.start_recording().unwrap();
        pipeline.try_start_concurrent_streaming(app).await;
        assert!(pipeline
            .inner
            .lock()
            .unwrap()
            .active_streaming_session
            .is_some());
        if mode == 2 {
            focus(false);
        }
        let result = pipeline.stop_and_transcribe_detailed().await;
        if mode == 2 {
            assert!(matches!(result, Err(PipelineError::Stt(_))));
            assert_eq!(pipeline.state(), PipelineState::Error);
            assert!(pipeline.clone_last_wav_bytes().is_some());
        } else {
            let result = result.unwrap();
            assert_eq!(result.final_text, "Synthetic committed text");
            assert_eq!(result.live_output_completed, mode == 0);
            if mode == 0 {
                assert_eq!(
                    arboard::Clipboard::new().unwrap().get_text().unwrap(),
                    "Synthetic committed text"
                );
            }
        }
    }
    focus(true);
    for (key, value) in keys.into_iter().zip(old) {
        if let Some(value) = value {
            store.set(key, value);
        } else {
            store.delete(key);
        }
    }
}

#[tokio::test]
async fn cancelling_streaming_connection_or_finalize_leaves_the_new_session_owned() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    for finalizing in [false, true] {
        let mut config = test_config_for_transcription();
        config.llm_config.enabled = false;
        let pipeline = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
        let mut provider = MockSttProvider::new("must not use batch");
        provider.realtime_only = true;
        pipeline.inject_stt_provider_for_tests(MOCK_PROVIDER, None, None, Arc::new(provider));
        pipeline.start_recording().unwrap();
        let (done, wait) = tokio::sync::oneshot::channel::<()>();
        if finalizing {
            let (audio_tx, _audio_rx) = tokio::sync::mpsc::channel(1);
            let (_partial_tx, partial_rx) = tokio::sync::mpsc::channel(1);
            pipeline.inner.lock().unwrap().active_streaming_session =
                Some(crate::stt::streaming::StreamingSttSession::new(
                    audio_tx,
                    partial_rx,
                    tokio::spawn(async move {
                        let _ = wait.await;
                        Ok("old result".into())
                    }),
                ));
        } else {
            pipeline.inner.lock().unwrap().pending_connection = Some(
                crate::stt::streaming::OwnedStreamingTask(tokio::spawn(async move {
                    let _ = wait.await;
                })),
            );
        }
        let mut old = Box::pin(pipeline.stop_and_transcribe_detailed());
        assert!(matches!(
            old.as_mut().poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        pipeline.cancel();
        pipeline.start_recording().unwrap();
        let new_epoch = pipeline.session_epoch();
        let _ = done.send(());
        assert!(matches!(old.await, Err(PipelineError::Cancelled)));
        assert_eq!(pipeline.state(), PipelineState::Recording);
        assert_eq!(pipeline.session_epoch(), new_epoch);
        assert!(!pipeline.get_cancel_token().unwrap().is_cancelled());
        pipeline.cancel();
    }
}

#[tokio::test(start_paused = true)]
async fn realtime_connection_can_arrive_late_or_time_out_without_batch_upload() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    for connects in [true, false] {
        let p = SharedPipeline::new_for_tests(
            test_config_for_transcription(),
            Box::new(FakeAudioCapture::new()),
        );
        let mut provider = MockSttProvider::new("must not batch");
        provider.realtime_only = true;
        p.inject_stt_provider_for_tests(MOCK_PROVIDER, None, None, Arc::new(provider));
        p.start_recording().unwrap();
        let (connected, wait) = tokio::sync::oneshot::channel::<()>();
        p.inner.lock().unwrap().pending_connection = Some(
            crate::stt::streaming::OwnedStreamingTask(tokio::spawn(async move {
                let _ = wait.await;
            })),
        );
        let mut pending = Box::pin(p.stop_and_transcribe_detailed());
        assert!(matches!(
            pending
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        if connects {
            let (tx, _rx) = tokio::sync::mpsc::channel(1);
            let (_tx, rx) = tokio::sync::mpsc::channel(1);
            p.inner.lock().unwrap().active_streaming_session =
                Some(crate::stt::streaming::StreamingSttSession::new(
                    tx,
                    rx,
                    tokio::spawn(async { Ok("late result".into()) }),
                ));
            connected.send(()).unwrap();
        }
        let result = pending.await;
        if connects {
            assert_eq!(result.unwrap().final_text, "late result");
            assert_eq!(p.state(), PipelineState::Idle);
        } else {
            assert!(
                matches!(result, Err(PipelineError::Stt(crate::stt::SttError::Config(message))) if message.contains("no batch fallback"))
            );
            assert_eq!(p.state(), PipelineState::Error);
        }
    }
}

#[tokio::test]
async fn journal_entry_point_requires_ownership_and_an_explicit_meeting_model() {
    use crate::recordings::options::{RecordingMode, RecordingPreferences};
    let pipeline = SharedPipeline::new_for_tests(
        test_config_for_transcription(),
        Box::new(FakeAudioCapture::new()),
    );
    let options = RecordingPreferences {
        mode: RecordingMode::Meeting,
        ..Default::default()
    };
    let result = pipeline
        .transcribe_journal_wav(vec![], None, None, &options)
        .await;
    assert!(matches!(result, Err(PipelineError::Config(message)) if message.contains("ownership")));
    pipeline.begin_recovery().unwrap();
    let result = pipeline
        .transcribe_journal_wav(vec![], None, None, &options)
        .await;
    assert!(
        matches!(result, Err(PipelineError::Config(message)) if message.contains("Select a meeting"))
    );
    pipeline.end_recovery();
}

#[test]
fn test_force_reset() {
    let config = PipelineConfig {
        stt_api_key: "test-key".to_string(),
        ..Default::default()
    };
    let pipeline = SharedPipeline::new(config);

    // Force reset should always work
    pipeline.force_reset();
    assert_eq!(pipeline.state(), PipelineState::Idle);
}

#[test]
fn test_cancel_from_recording_transitions_to_idle() {
    let pipeline = SharedPipeline::new(PipelineConfig::default());
    let token = CancellationToken::new();

    // Given a pipeline in Recording with an active cancel token
    set_state_for_test(&pipeline, PipelineState::Recording, Some(token.clone()));

    // When cancellation is requested
    pipeline.cancel();

    // Then the pipeline resets to Idle and the token is cancelled
    assert_eq!(pipeline.state(), PipelineState::Idle);
    assert!(token.is_cancelled());
    assert!(pipeline.get_cancel_token().is_none());
}

#[test]
fn test_cancel_from_transcribing_transitions_to_idle() {
    let pipeline = SharedPipeline::new(PipelineConfig::default());
    let token = CancellationToken::new();

    // Given a pipeline in Transcribing with an active cancel token
    set_state_for_test(&pipeline, PipelineState::Transcribing, Some(token.clone()));

    // When cancellation is requested
    pipeline.cancel();

    // Then the pipeline resets to Idle and the token is cancelled
    assert_eq!(pipeline.state(), PipelineState::Idle);
    assert!(token.is_cancelled());
}

#[tokio::test]
async fn cancelled_transcription_cannot_reset_a_new_recording() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let pipeline = SharedPipeline::new_for_tests(
        test_config_for_transcription(),
        Box::new(FakeAudioCapture::new()),
    );
    pipeline.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(
            MockSttProvider::new("old transcript").with_behavior(MockBehavior {
                delay: Some(Duration::from_secs(3600)),
                error: None,
            }),
        ),
    );
    pipeline.start_recording().unwrap();
    let mut old = Box::pin(pipeline.stop_and_transcribe_detailed());
    assert!(matches!(
        old.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    assert_eq!(pipeline.state(), PipelineState::Transcribing);
    let old_epoch = pipeline.session_epoch().unwrap();
    pipeline.cancel();
    pipeline
        .start_recording_with_output(true, None, false)
        .unwrap();
    let new_token = pipeline.get_cancel_token().unwrap();
    assert_ne!(pipeline.session_epoch(), Some(old_epoch));
    assert!(matches!(old.await, Err(PipelineError::Cancelled)));
    assert_eq!(pipeline.state(), PipelineState::Recording);
    assert!(!new_token.is_cancelled());
    assert!(pipeline.get_cancel_token().is_some());
    assert!(pipeline.is_history_only_recording());
    pipeline.cancel();
}

#[tokio::test]
async fn cancelled_rewrite_cannot_change_the_next_recording_or_its_logs() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let logs = crate::request_log::RequestLogStore::new();
    let mut config = test_config_for_transcription();
    config.llm_config = mock_llm_config(true, Vec::new());
    insert_mock_llm_api_key(&mut config);
    config.request_log_store = Some(logs.clone());
    let pipeline = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    pipeline.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(MockSttProvider::new("old text")),
    );
    pipeline.inject_llm_provider_for_tests(
        MOCK_PROVIDER,
        None,
        Arc::new(
            MockLlmProvider::new("old rewrite").with_behavior(MockBehavior {
                delay: Some(Duration::from_secs(3600)),
                error: None,
            }),
        ),
    );
    logs.start_request(MOCK_PROVIDER.into(), None);
    pipeline.start_recording().unwrap();
    let mut old = Box::pin(pipeline.stop_and_transcribe_detailed());
    assert!(matches!(
        old.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    assert_eq!(pipeline.state(), PipelineState::Rewriting);
    pipeline.cancel();
    pipeline.start_recording().unwrap();
    let new_id = logs.start_request("next-provider".into(), None);
    assert!(matches!(old.await, Err(PipelineError::Cancelled)));
    assert_eq!(pipeline.state(), PipelineState::Recording);
    let current = logs.get_logs(Some(1)).pop().unwrap();
    assert_eq!(current.id, new_id);
    assert!(current.llm_provider.is_none());
    assert!(current.preset_id.is_none());
    assert!(current.rewrite_clipboard_context.is_none());
    pipeline.cancel();
}

#[test]
fn stale_completion_and_rewrite_callbacks_leave_newer_session_untouched() {
    use super::transcription_flow::TranscriptionCallbacks;
    let pipeline = SharedPipeline::new_for_tests(
        test_config_for_transcription(),
        Box::new(FakeAudioCapture::new()),
    );
    pipeline.start_recording().unwrap();
    let old = pipeline.get_cancel_token().unwrap();
    let old_live_output = pipeline.inner.lock().unwrap().live_output_active.clone();
    let callbacks = PipelineCallbacks {
        inner: pipeline.inner.clone(),
        cancel: old.clone(),
    };
    pipeline.force_reset();
    pipeline.start_recording().unwrap();
    let current = pipeline.get_cancel_token().unwrap();
    old_live_output.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(!pipeline
        .inner
        .lock()
        .unwrap()
        .live_output_active
        .load(std::sync::atomic::Ordering::SeqCst));
    pipeline
        .set_session_preset_lock(Some("new-profile".into()), Some("new-preset".into()))
        .unwrap();
    assert!(matches!(
        pipeline.take_session_preset_lock(&old),
        Err(PipelineError::Cancelled)
    ));
    assert_eq!(
        pipeline.peek_session_preset_lock(),
        Some((Some("new-profile".into()), "new-preset".into()))
    );
    assert_eq!(
        pipeline
            .take_session_preset_lock(&current)
            .unwrap()
            .unwrap()
            .preset_id,
        "new-preset"
    );
    // Even if the new recording is already transcribing, old callbacks cannot
    // change its phase, clear its cancel token, or signal STT completion.
    pipeline.inner.lock().unwrap().state = PipelineState::Transcribing;
    callbacks.transition_to_routing();
    callbacks.transition_to_rewriting();
    assert_eq!(pipeline.state(), PipelineState::Transcribing);
    pipeline.inner.lock().unwrap().state = PipelineState::Routing;
    callbacks.transition_from_routing();
    assert_eq!(pipeline.state(), PipelineState::Routing);
    assert!(matches!(
        pipeline.mark_stt_complete(&old, "stale"),
        Err(PipelineError::Cancelled)
    ));
    pipeline
        .finish_failed_stt_attempt(&old, &PipelineError::Config("late failure".into()))
        .unwrap();
    assert_eq!(pipeline.state(), PipelineState::Routing);
    assert!(!current.is_cancelled());
    assert!(!pipeline.inner.lock().unwrap().stt_complete);
    pipeline.mark_stt_complete(&current, "current").unwrap();
    assert!(pipeline.inner.lock().unwrap().stt_complete);
    pipeline
        .finish_failed_stt_attempt(&current, &PipelineError::Cancelled)
        .unwrap();
    assert_eq!(pipeline.state(), PipelineState::Idle);
    assert!(current.is_cancelled());
}

#[test]
fn cancelling_home_capture_preserves_audio_and_mode_without_submitting() {
    use crate::audio_capture::journal::{read_chunk, Journal};
    use crate::recordings::options::{MeetingModel, RecordingMode, RecordingPreferences};

    for mode in [RecordingMode::Dictation, RecordingMode::Meeting] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("recording.pcm");
        let mut journal = Journal::create(&path, 16000, 1).unwrap();
        journal.append(&[0.25, -0.25], 16000, 1).unwrap();
        journal.finish().unwrap();
        drop(journal);
        let preferences = RecordingPreferences {
            mode,
            meeting_model: Some(MeetingModel {
                provider: "openai".into(),
                model: "gpt-4o-transcribe-diarize".into(),
                use_managed: false,
            }),
            language_hint: None,
        };
        preferences.save_journal(&path).unwrap();
        let capture = FakeAudioCapture::new();
        let calls = capture.cancellation_calls.clone();
        let pipeline = SharedPipeline::new_for_tests(
            test_config_with_max_recording_bytes(),
            Box::new(capture),
        );
        pipeline
            .start_recording_with_output(true, Some(path.clone()), false)
            .unwrap();
        let token = pipeline.get_cancel_token().unwrap();
        pipeline.set_recording_paused(true).unwrap();

        pipeline.cancel();

        assert_eq!(*calls.lock().unwrap(), vec!["stop", "finish"]);
        assert!(token.is_cancelled());
        assert_eq!(pipeline.state(), PipelineState::Idle);
        assert_eq!(read_chunk(&path, 0, 10).unwrap().2, vec![0.25, -0.25]);
        let recovered = RecordingPreferences::load_journal(&path).unwrap();
        assert_eq!(recovered.mode, preferences.mode);
        assert_eq!(recovered.meeting_model, preferences.meeting_model);
        assert!(pipeline.clone_last_wav_bytes().is_none());
        assert!(!pipeline.is_recovering());

        // A subsequent ordinary shortcut session must not inherit this journal.
        pipeline.start_recording().unwrap();
        assert!(!pipeline.is_history_only_recording());
        assert!(pipeline.recovery_path().is_none());
        pipeline.cancel();
        assert!(path.exists());
        assert!(path.with_extension("options.json").exists());
        assert_eq!(
            *calls.lock().unwrap(),
            vec!["stop", "finish", "stop", "discard"]
        );
    }
}

#[test]
fn cancelling_home_capture_reports_sync_failure_without_discarding() {
    let mut capture = FakeAudioCapture::new();
    capture.fail_recovery_finish = true;
    let calls = capture.cancellation_calls.clone();
    let pipeline =
        SharedPipeline::new_for_tests(test_config_with_max_recording_bytes(), Box::new(capture));
    pipeline
        .start_recording_with_output(true, None, false)
        .unwrap();
    let token = pipeline.get_cancel_token().unwrap();

    pipeline.cancel();

    assert!(token.is_cancelled());
    assert_eq!(pipeline.state(), PipelineState::Error);
    assert_eq!(*calls.lock().unwrap(), vec!["stop", "finish"]);
}

#[test]
fn cancelling_shortcut_capture_does_not_create_recovery_persistence() {
    let capture = FakeAudioCapture::new();
    let calls = capture.cancellation_calls.clone();
    let pipeline =
        SharedPipeline::new_for_tests(test_config_with_max_recording_bytes(), Box::new(capture));
    pipeline.start_recording().unwrap();
    pipeline.cancel();

    assert_eq!(pipeline.state(), PipelineState::Idle);
    assert!(pipeline.recovery_path().is_none());
    assert_eq!(*calls.lock().unwrap(), vec!["stop", "discard"]);
}

#[test]
fn test_stop_recording_transitions_to_idle() {
    let pipeline = SharedPipeline::new(PipelineConfig::default());
    let token = CancellationToken::new();

    // Given a pipeline marked as Recording
    set_state_for_test(&pipeline, PipelineState::Recording, Some(token));

    // When stopping the recording
    let result = pipeline.stop_recording();

    // Then it resets to Idle and captures a WAV buffer
    assert!(result.is_ok());
    assert_eq!(pipeline.state(), PipelineState::Idle);
    assert!(pipeline.clone_last_wav_bytes().is_some());
    let first = pipeline.clone_last_wav_bytes().unwrap();
    let second = pipeline.clone_last_wav_bytes().unwrap();
    assert!(
        Arc::ptr_eq(&first, &second),
        "audio snapshots must not duplicate the WAV allocation"
    );
    assert!(pipeline.get_cancel_token().is_none());
}

#[test]
fn ocr_session_survives_reset_to_idle() {
    let config = test_config_with_max_recording_bytes();
    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));

    p.begin_ocr_session("req-1".to_string());

    // Seed a completed OCR result so status reports "done".
    {
        let mut inner = p.inner.lock().unwrap();
        inner.state = PipelineState::Recording;
        inner.ocr.result = Some(crate::ocr::OcrResult {
            text: "hello".to_string(),
            provider: "test".to_string(),
            model: "test".to_string(),
        });
        inner.reset_to_idle();
        assert_eq!(inner.state, PipelineState::Idle);
        assert_eq!(inner.ocr.session_id.as_deref(), Some("req-1"));
        assert!(inner.ocr.result.is_some());
    }

    assert_eq!(p.get_ocr_status(), "done");
    assert_eq!(p.ocr_session_id().as_deref(), Some("req-1"));
}

#[test]
fn recording_output_mode_is_owned_by_the_session() {
    let config = test_config_with_max_recording_bytes();
    let pipeline = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    pipeline
        .start_recording_with_output(true, None, false)
        .unwrap();
    pipeline.set_recording_paused(true).unwrap();
    assert!(pipeline.is_recording_paused());
    pipeline.set_recording_paused(false).unwrap();
    assert!(!pipeline.is_recording_paused());
    assert!(pipeline.is_history_only_recording());
    assert!(pipeline.start_recording().is_err());
    assert!(pipeline.is_history_only_recording());
    pipeline.cancel();
    pipeline.start_recording().unwrap();
    assert!(!pipeline.is_history_only_recording());
    assert!(pipeline.set_recording_paused(true).is_err());
}

#[test]
fn recovery_owns_pipeline_between_chunks_and_cancels_without_new_recording() {
    let pipeline = SharedPipeline::new_for_tests(
        test_config_with_max_recording_bytes(),
        Box::new(FakeAudioCapture::new()),
    );
    let token = pipeline.begin_recovery().unwrap();
    assert!(pipeline.is_recovering());
    assert!(pipeline.begin_recovery().is_err());
    assert!(pipeline.start_recording().is_err());
    pipeline.cancel();
    assert!(token.is_cancelled());
    assert!(pipeline.start_recording().is_err());
    pipeline.end_recovery();
    pipeline.start_recording().unwrap();
    assert!(pipeline.begin_recovery().is_err());
    pipeline.cancel();
}

#[test]
fn recovery_retry_clears_prior_error_before_cancellable_preparation() {
    let pipeline = SharedPipeline::new_for_tests(
        test_config_with_max_recording_bytes(),
        Box::new(FakeAudioCapture::new()),
    );
    set_state_for_test(&pipeline, PipelineState::Error, None);

    let token = pipeline.begin_recovery().unwrap();
    assert_eq!(pipeline.state(), PipelineState::Idle);
    assert!(pipeline.is_recovering());
    pipeline.cancel();
    assert!(token.is_cancelled());
    assert!(pipeline.is_recovering());
    pipeline.end_recovery();
    assert!(!pipeline.is_recovering());
}

#[test]
fn end_ocr_session_clears_ocr_state() {
    let config = test_config_with_max_recording_bytes();
    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));

    p.begin_ocr_session("req-2".to_string());
    {
        let mut inner = p.inner.lock().unwrap();
        inner.ocr.result = Some(crate::ocr::OcrResult {
            text: "hello".to_string(),
            provider: "test".to_string(),
            model: "test".to_string(),
        });
    }

    assert_eq!(p.get_ocr_status(), "done");

    p.end_ocr_session_if_matches("req-2");
    assert_eq!(p.ocr_session_id(), None);
    assert_eq!(p.get_ocr_status(), "not_started");
}

#[tokio::test]
async fn get_ocr_result_without_task_keeps_status_not_started() {
    let config = test_config_with_max_recording_bytes();
    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));

    p.begin_ocr_session("req-no-task".to_string());

    let result = p
        .get_ocr_result_with_timeout(std::time::Duration::from_millis(1), None)
        .await;

    assert!(result.is_none());
    assert_eq!(p.ocr_session_id().as_deref(), Some("req-no-task"));
    assert_eq!(p.get_ocr_status(), "not_started");
    assert!(!p.inner.lock().unwrap().ocr.awaiting);
}

#[tokio::test]
async fn cancelled_transcription_cannot_consume_newer_ocr() {
    let pipeline = SharedPipeline::new_for_tests(
        test_config_for_transcription(),
        Box::new(FakeAudioCapture::new()),
    );
    pipeline.begin_ocr_session("new-request".into());
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    // This flag belongs to the newer session; the old caller must not even
    // enter the no-task cleanup path, which clears it.
    pipeline.inner.lock().unwrap().ocr.awaiting = true;
    assert!(pipeline
        .get_ocr_result_with_timeout(Duration::from_secs(1), Some(&cancelled))
        .await
        .is_none());
    assert!(pipeline.inner.lock().unwrap().ocr.awaiting);
    assert_eq!(pipeline.ocr_session_id().as_deref(), Some("new-request"));
    assert!(pipeline
        .get_ocr_result_with_timeout(Duration::from_secs(1), Some(&CancellationToken::new()))
        .await
        .is_none());
    assert!(!pipeline.inner.lock().unwrap().ocr.awaiting);
}

#[tokio::test]
async fn awaited_ocr_task_cannot_publish_after_session_superseded() {
    let config = test_config_with_max_recording_bytes();
    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));

    p.begin_ocr_session("req-old".to_string());

    let (finish_tx, finish_rx) = tokio::sync::oneshot::channel::<()>();
    let handle = tokio::spawn(async move {
        let _ = finish_rx.await;
        Ok(crate::ocr::OcrResult {
            text: "stale OCR text".to_string(),
            provider: "test".to_string(),
            model: "test".to_string(),
        })
    });
    let abort_handle = handle.abort_handle();
    {
        let mut inner = p.inner.lock().unwrap();
        inner.ocr.abort_handle = Some(abort_handle);
        inner.ocr.task = Some(OcrTaskHandle::new(
            Some("req-old".to_string()),
            Some("req-old".to_string()),
            handle,
        ));
    }

    let waiter_pipeline = p.clone();
    let waiter = tokio::spawn(async move {
        waiter_pipeline
            .get_ocr_result_with_timeout(std::time::Duration::from_secs(30), None)
            .await
    });

    for _ in 0..16 {
        if p.inner.lock().unwrap().ocr.awaiting {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(p.inner.lock().unwrap().ocr.awaiting);

    p.begin_ocr_session("req-new".to_string());
    let _ = finish_tx.send(());

    let result = waiter.await.expect("OCR waiter task should not panic");
    assert!(result.is_none());
    assert_eq!(p.ocr_session_id().as_deref(), Some("req-new"));
    assert_eq!(p.get_ocr_status(), "not_started");
}

#[tokio::test]
async fn force_reset_aborts_in_flight_ocr_task() {
    struct NotifyOnDrop(Option<tokio::sync::oneshot::Sender<()>>);

    impl Drop for NotifyOnDrop {
        fn drop(&mut self) {
            if let Some(tx) = self.0.take() {
                let _ = tx.send(());
            }
        }
    }

    let config = test_config_with_max_recording_bytes();
    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.begin_ocr_session("req-reset".to_string());

    let (dropped_tx, dropped_rx) = tokio::sync::oneshot::channel::<()>();
    let drop_guard = NotifyOnDrop(Some(dropped_tx));
    let handle = tokio::spawn(async move {
        let _drop_guard = drop_guard;
        std::future::pending::<Result<crate::ocr::OcrResult, String>>().await
    });
    let abort_handle = handle.abort_handle();
    {
        let mut inner = p.inner.lock().unwrap();
        inner.ocr.abort_handle = Some(abort_handle);
        inner.ocr.task = Some(OcrTaskHandle::new(
            Some("req-reset".to_string()),
            Some("req-reset".to_string()),
            handle,
        ));
    }

    p.force_reset();

    tokio::time::timeout(std::time::Duration::from_secs(1), dropped_rx)
        .await
        .expect("OCR task should be dropped after abort")
        .expect("drop notification should be sent");
    let inner = p.inner.lock().unwrap();
    assert_eq!(inner.ocr.session_id, None);
    assert!(inner.ocr.task.is_none());
    assert!(inner.ocr.abort_handle.is_none());
    assert!(inner.ocr.result.is_none());
}

#[tokio::test]
async fn force_reset_aborts_awaited_ocr_task() {
    struct NotifyOnDrop(Option<tokio::sync::oneshot::Sender<()>>);

    impl Drop for NotifyOnDrop {
        fn drop(&mut self) {
            if let Some(tx) = self.0.take() {
                let _ = tx.send(());
            }
        }
    }

    let config = test_config_with_max_recording_bytes();
    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.begin_ocr_session("req-awaiting-reset".to_string());

    let (dropped_tx, dropped_rx) = tokio::sync::oneshot::channel::<()>();
    let drop_guard = NotifyOnDrop(Some(dropped_tx));
    let handle = tokio::spawn(async move {
        let _drop_guard = drop_guard;
        std::future::pending::<Result<crate::ocr::OcrResult, String>>().await
    });
    let abort_handle = handle.abort_handle();
    {
        let mut inner = p.inner.lock().unwrap();
        inner.ocr.abort_handle = Some(abort_handle);
        inner.ocr.task = Some(OcrTaskHandle::new(
            Some("req-awaiting-reset".to_string()),
            Some("req-awaiting-reset".to_string()),
            handle,
        ));
    }

    let waiter_pipeline = p.clone();
    let waiter = tokio::spawn(async move {
        waiter_pipeline
            .get_ocr_result_with_timeout(std::time::Duration::from_secs(30), None)
            .await
    });

    for _ in 0..16 {
        if p.inner.lock().unwrap().ocr.awaiting {
            break;
        }
        tokio::task::yield_now().await;
    }
    {
        let inner = p.inner.lock().unwrap();
        assert!(inner.ocr.awaiting);
        assert!(inner.ocr.task.is_none());
        assert!(inner.ocr.abort_handle.is_some());
    }

    p.force_reset();

    tokio::time::timeout(std::time::Duration::from_secs(1), dropped_rx)
        .await
        .expect("awaited OCR task should be dropped after abort")
        .expect("drop notification should be sent");
    let result = waiter.await.expect("OCR waiter task should not panic");
    assert!(result.is_none());

    let inner = p.inner.lock().unwrap();
    assert_eq!(inner.ocr.session_id, None);
    assert!(inner.ocr.task.is_none());
    assert!(inner.ocr.abort_handle.is_none());
    assert!(inner.ocr.result.is_none());
    assert!(!inner.ocr.awaiting);
}

#[test]
fn pipeline_can_start_and_stop_without_cpal() {
    let config = test_config_with_max_recording_bytes();

    let fake = FakeAudioCapture::new();
    let meter_handle = fake.shared_level_meter();
    let waveform_handle = fake.shared_waveform_meter();

    let p = SharedPipeline::new_for_tests(config, Box::new(fake));

    assert_eq!(p.try_state(), Some(PipelineState::Idle));
    p.start_recording().expect("start recording should succeed");
    assert_eq!(p.try_state(), Some(PipelineState::Recording));

    // Simulate realtime meter updates without needing an actual CPAL callback.
    let s0 = p.audio_level_snapshot_fast();
    meter_handle.set_for_tests(0.25, 0.5);
    let s1 = p.audio_level_snapshot_fast();
    assert!(s1.seq > s0.seq);
    assert!((s1.rms - 0.25).abs() < 1e-6);
    assert!((s1.peak - 0.5).abs() < 1e-6);

    // Same idea for the waveform meter: simulate samples without needing CPAL.
    let w0 = p.audio_waveform_snapshot_fast();
    waveform_handle.set_from_samples_for_tests(&[0.0, -0.25, 0.5, 1.0], 1);
    let w1 = p.audio_waveform_snapshot_fast();
    assert!(w1.seq > w0.seq);

    let wav = p.stop_recording().expect("stop recording should succeed");
    assert_eq!(wav, vec![1, 2, 3]);
    assert_eq!(p.try_state(), Some(PipelineState::Idle));

    p.start_recording().unwrap();
    let (before, after) = p.stop_recording_before_after().unwrap();
    assert_eq!(before, vec![9]);
    assert_eq!(after, vec![8]);
    let retained = p.clone_last_wav_bytes().unwrap();
    assert_eq!(retained.as_slice(), after.as_slice());
    assert!(Arc::ptr_eq(&retained, &p.clone_last_wav_bytes().unwrap()));
    assert_eq!(p.try_state(), Some(PipelineState::Idle));
}

#[test]
fn start_recording_failure_transitions_pipeline_to_error() {
    let config = test_config_with_max_recording_bytes();
    let pipeline = SharedPipeline::new_for_tests(config, Box::new(FailingStartAudioCapture::new()));

    let result = pipeline.start_recording();

    assert!(matches!(
        result,
        Err(crate::pipeline::PipelineError::AudioCapture(
            AudioCaptureError::NoInputDevice
        ))
    ));
    assert_eq!(pipeline.state(), PipelineState::Error);
}

#[tokio::test]
async fn pipeline_can_transcribe_without_network_or_hardware() {
    // Given: a pipeline with fake audio capture (no CPAL device) and a fake STT provider
    // injected into the provider cache (no network).
    let config = test_config_for_transcription();

    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(MockSttProvider::new("hello from tests")),
    );

    p.start_recording().expect("start recording should succeed");

    // When: we stop and transcribe
    let result = p
        .stop_and_transcribe_detailed()
        .await
        .expect("stop/transcribe should succeed");

    // Then: we get a transcript without hitting any real IO.
    assert_eq!(result.stt_text, "hello from tests");
    assert_eq!(result.final_text, "hello from tests");
    assert_eq!(p.state(), PipelineState::Idle);
}

#[tokio::test]
async fn skipped_quiet_audio_has_no_claimed_language_or_provider_request() {
    for speech_detected in [Some(false), None] {
        let mut capture = FakeAudioCapture::new();
        capture.diagnostics_override = Some(AudioCaptureDiagnostics {
            speech_detected,
            ..FakeAudioCapture::diagnostics()
        });
        let mut config = test_config_for_transcription();
        config.quiet_audio_gate_enabled = true;
        config.quiet_audio_require_speech = true;
        config.quiet_audio_min_duration_secs = 1.0;
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut provider = MockSttProvider::new("must not transcribe");
        provider.batch_calls = calls.clone();
        let pipeline = SharedPipeline::new_for_tests(config, Box::new(capture));
        pipeline.inject_stt_provider_for_tests(MOCK_PROVIDER, None, None, Arc::new(provider));
        pipeline.start_recording().unwrap();
        let result = pipeline.stop_and_transcribe_detailed().await.unwrap();
        assert!(result.final_text.is_empty());
        assert_eq!(result.stt_language_hint, None);
        assert!(result.stt_detected_languages.is_empty());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(pipeline.state(), PipelineState::Idle);
        assert!(matches!(
            result.llm_outcome,
            LlmOutcome::NotAttempted(
                LlmNotAttemptedReason::NoSpeechDetectedByVad
                    | LlmNotAttemptedReason::QuietAudioGate
            )
        ));
    }
}

#[tokio::test]
async fn meeting_transcription_bypasses_only_dictation_size_limit_and_requires_ownership() {
    let mut config = test_config_for_transcription();
    config.max_recording_bytes = 1024;
    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(MockSttProvider::new("one final transcript")),
    );
    let mut audio = crate::audio_capture::AudioBuffer::new(16000, 1, 1.0);
    audio.append(&vec![0.25; 16000]);
    let wav = audio.to_wav_bytes().unwrap();
    assert!(matches!(
        p.transcribe_wav_bytes_detailed(wav.clone()).await,
        Err(PipelineError::RecordingTooLarge(..))
    ));
    assert!(p
        .transcribe_journal_wav(wav.clone(), None, None, &Default::default())
        .await
        .is_err());
    p.begin_recovery().unwrap();
    let wav = Arc::new(wav);
    let result = p
        .transcribe_journal_wav(wav.clone(), None, None, &Default::default())
        .await
        .unwrap();
    assert_eq!(result.stt_text, "one final transcript");
    assert_eq!(result.final_text, "one final transcript");
    assert_eq!(p.state(), PipelineState::Idle);
    assert!(p.is_recovering());
    assert_eq!(
        Arc::strong_count(&wav),
        1,
        "completed recovery must release its shared audio"
    );
    p.end_recovery();
}

#[tokio::test]
async fn invalid_journal_audio_fails_without_releasing_recovery_ownership() {
    let p = SharedPipeline::new_for_tests(
        test_config_for_transcription(),
        Box::new(FakeAudioCapture::new()),
    );
    p.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(MockSttProvider::new("must not upload")),
    );
    p.begin_recovery().unwrap();
    let result = p
        .transcribe_journal_wav(vec![0, 1, 2], None, None, &Default::default())
        .await;
    assert!(matches!(result, Err(PipelineError::Config(_))));
    assert_eq!(p.state(), PipelineState::Error);
    assert!(p.is_recovering());
    p.end_recovery();
    assert!(!p.is_recovering());
}

#[tokio::test]
async fn disk_sources_cannot_accidentally_enter_the_in_memory_batch_path() {
    let p = SharedPipeline::new_for_tests(
        test_config_for_transcription(),
        Box::new(FakeAudioCapture::new()),
    );
    p.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(MockSttProvider::new("must not upload")),
    );
    let source = TranscriptionAudio::File {
        path: "not-opened.wav".into(),
        bytes: 44,
        modified: std::time::SystemTime::UNIX_EPOCH,
        duration: 0.0,
    };
    let result = p
        .transcribe_saved_audio(
            source,
            None,
            None,
            None,
            None,
            None,
            SavedAudioMode::Dictation,
        )
        .await;
    assert!(
        matches!(result, Err(PipelineError::Config(message)) if message == "Saved files require chunked transcription")
    );
    assert!(p.clone_last_wav_bytes().is_none());
    assert_eq!(
        p.state(),
        PipelineState::Error,
        "invalid input must not leave a transcription in progress"
    );
}

#[tokio::test]
async fn managed_auth_failure_without_a_desktop_session_releases_transcription() {
    let mut config = test_config_for_transcription();
    config.managed_inference_enabled = true;
    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    let mut provider = MockSttProvider::new("must not succeed");
    provider.behavior.error = Some("401 auth_invalid_token".into());
    let result = p
        .run_batch_stt_request(
            Arc::new(provider),
            MOCK_PROVIDER,
            None,
            None,
            Some("synthetic-managed-token".into()),
            &[1, 2, 3],
            &crate::stt::RetryConfig::default(),
            Duration::from_secs(1),
            &CancellationToken::new(),
            "test",
            "retry_transcription",
            None,
        )
        .await;
    assert!(
        matches!(result, Err(PipelineError::Config(message)) if message == "Managed auth refresh unavailable: app handle missing")
    );
    assert_eq!(p.state(), PipelineState::Error);
}

#[tokio::test]
async fn meeting_managed_choice_never_falls_back_to_a_user_key_when_access_is_unavailable() {
    use crate::recordings::options::{MeetingModel, RecordingMode, RecordingPreferences};
    let mut config = test_config_for_transcription();
    config.managed_stt_preferred = false;
    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    // A direct provider is ready in the cache. Choosing Managed may not use it.
    p.inject_stt_provider_for_tests(
        "openai",
        Some("gpt-4o-transcribe-diarize"),
        None,
        Arc::new(MockSttProvider::new("Must not use a different route")),
    );
    let mut audio = crate::audio_capture::AudioBuffer::new(16000, 1, 1.0);
    audio.append(&vec![0.25; 16000]);
    let options = RecordingPreferences {
        mode: RecordingMode::Meeting,
        meeting_model: Some(MeetingModel {
            provider: "openai".into(),
            model: "gpt-4o-transcribe-diarize".into(),
            use_managed: true,
        }),
        language_hint: None,
    };
    p.begin_recovery().unwrap();
    let error = p
        .transcribe_journal_wav(audio.to_wav_bytes().unwrap(), None, None, &options)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("Managed meeting transcription is unavailable"));
    assert!(!p.config().managed_stt_preferred);
    assert!(p.is_recovering());
    p.end_recovery();
}

#[tokio::test]
async fn settings_audio_test_keeps_the_explicit_dictation_language() {
    let mut config = test_config_for_transcription();
    config.stt_language = Some("en".into());
    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        Some("en"),
        Arc::new(MockSttProvider::new("English settings test")),
    );
    p.start_recording().unwrap();
    p.stop_recording().unwrap();
    assert_eq!(
        p.transcribe_last_audio_for_profile(None).await.unwrap(),
        "English settings test"
    );
    assert_eq!(p.config().stt_language.as_deref(), Some("en"));
    assert_eq!(p.state(), PipelineState::Idle);
}

#[tokio::test]
async fn saved_meeting_language_never_follows_later_dictation_settings() {
    use crate::recordings::options::{MeetingModel, RecordingMode, RecordingPreferences};
    for hint in [None, Some("auto"), Some("yi"), Some("he")] {
        let mut config = test_config_for_transcription();
        config.stt_language = Some("en".into());
        let logs = crate::request_log::RequestLogStore::new();
        logs.start_request(MOCK_PROVIDER.into(), Some("snapshot-model".into()));
        logs.with_current(|log| log.set_stt_response(serde_json::json!({"language_code":"yi"})));
        config.request_log_store = Some(logs);
        let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
        let expected = hint.filter(|value| *value != "auto");
        p.inject_stt_provider_for_tests(
            MOCK_PROVIDER,
            Some("snapshot-model"),
            expected,
            Arc::new(MockSttProvider::new("Snapshot language")),
        );
        let options = RecordingPreferences {
            mode: RecordingMode::Meeting,
            meeting_model: Some(MeetingModel {
                provider: MOCK_PROVIDER.into(),
                model: "snapshot-model".into(),
                use_managed: false,
            }),
            language_hint: hint.map(String::from),
        };
        let mut audio = crate::audio_capture::AudioBuffer::new(16000, 1, 1.0);
        audio.append(&vec![0.25; 16000]);
        for later_hint in ["es", "fr"] {
            p.inner.lock().unwrap().config.stt_language = Some(later_hint.into());
            p.begin_recovery().unwrap();
            let result = p
                .transcribe_journal_wav(audio.to_wav_bytes().unwrap(), None, None, &options)
                .await
                .unwrap();
            assert_eq!(result.stt_text, "Snapshot language");
            assert_eq!(result.stt_detected_languages, ["yi"]);
            assert_eq!(
                result.stt_language_hint.as_deref(),
                Some(hint.unwrap_or("auto"))
            );
            assert_eq!(p.config().stt_language.as_deref(), Some(later_hint));
            assert_eq!(options.language_hint.as_deref(), hint);
            p.end_recovery();
        }
    }
}

#[tokio::test]
async fn meeting_mode_never_rewrites_even_when_dictation_rewriting_is_enabled() {
    use crate::recordings::options::{MeetingModel, RecordingMode, RecordingPreferences};
    let mut config = test_config_for_transcription();
    config.llm_config = mock_llm_config(true, Vec::new());
    insert_mock_llm_api_key(&mut config);
    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        Some("meeting-model"),
        None,
        Arc::new(MockSttProvider::new("Unchanged meeting")),
    );
    p.inject_llm_provider_for_tests(
        MOCK_PROVIDER,
        None,
        Arc::new(MockLlmProvider::new("Must not run")),
    );
    let mut audio = crate::audio_capture::AudioBuffer::new(16000, 1, 1.0);
    audio.append(&vec![0.25; 16000]);
    let options = RecordingPreferences {
        mode: RecordingMode::Meeting,
        meeting_model: Some(MeetingModel {
            provider: MOCK_PROVIDER.into(),
            model: "meeting-model".into(),
            use_managed: false,
        }),
        language_hint: None,
    };
    p.inner.lock().unwrap().config.managed_stt_preferred = true;
    p.begin_recovery().unwrap();
    let result = p
        .transcribe_journal_wav(audio.to_wav_bytes().unwrap(), None, None, &options)
        .await
        .unwrap();
    assert_eq!(result.final_text, "Unchanged meeting");
    assert!(
        p.config().managed_stt_preferred,
        "Meeting must not change Dictation's route"
    );
    assert!(!result.llm_attempted());
    assert_eq!(
        result.llm_outcome,
        crate::pipeline::LlmOutcome::NotAttempted(
            crate::pipeline::LlmNotAttemptedReason::MeetingMode
        )
    );
    assert!(p.is_recovering());
    p.end_recovery();
}

#[tokio::test]
async fn pipeline_can_transcribe_and_rewrite_without_network_or_hardware() {
    // Given: a pipeline with fake audio capture, fake STT, AND fake LLM providers.
    let mut config = test_config_for_transcription();
    config.llm_config = mock_llm_config(true, Vec::new());
    // Insert fake API key so the pipeline doesn't bail early.
    insert_mock_llm_api_key(&mut config);

    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(MockSttProvider::new("hello from stt")),
    );
    p.inject_llm_provider_for_tests(
        MOCK_PROVIDER,
        None,
        Arc::new(MockLlmProvider::new("Hello from LLM")),
    );

    p.start_recording().expect("start recording should succeed");

    // When: we stop and transcribe (with rewrite enabled)
    let result = p
        .stop_and_transcribe_detailed()
        .await
        .expect("stop/transcribe should succeed");

    // Then: the final output should be the LLM-rewritten text, not the raw STT.
    assert_eq!(result.stt_text, "hello from stt");
    assert_eq!(result.final_text, "Hello from LLM");
    assert!(result.llm_attempted());
    assert_eq!(result.llm_outcome, crate::pipeline::LlmOutcome::Succeeded);
    assert_eq!(p.state(), PipelineState::Idle);
}

#[tokio::test]
async fn rewrite_disabled_falls_back_to_stt_text() {
    // Given: a pipeline with LLM rewrite explicitly disabled.
    let mut config = test_config_for_transcription();
    config.llm_config = mock_llm_config(false, Vec::new());

    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(MockSttProvider::new("raw stt transcript")),
    );
    // NOTE: we do NOT inject an LLM provider — rewrite should not be attempted.

    p.start_recording().expect("start recording should succeed");

    // When: we stop and transcribe
    let result = p
        .stop_and_transcribe_detailed()
        .await
        .expect("stop/transcribe should succeed");

    // Then: final_text should match stt_text (no rewrite).
    assert_eq!(result.stt_text, "raw stt transcript");
    assert_eq!(result.final_text, "raw stt transcript");
    assert!(!result.llm_attempted());
    assert!(matches!(
        result.llm_outcome,
        crate::pipeline::LlmOutcome::NotAttempted(_)
    ));
    assert_eq!(p.state(), PipelineState::Idle);
}

#[tokio::test]
async fn stt_error_propagates_correctly() {
    // Given: a pipeline with an STT provider that will fail.
    let config = test_config_for_transcription();

    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(MockSttProvider::new("unused").with_behavior(MockBehavior {
            error: Some("Simulated API failure".to_string()),
            ..Default::default()
        })),
    );

    p.start_recording().expect("start recording should succeed");

    // When: we stop and transcribe
    let result = p.stop_and_transcribe_detailed().await;

    // Then: we get an error, not a success.
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, crate::pipeline::PipelineError::Stt(_)),
        "Expected STT error, got: {:?}",
        err
    );
    // Pipeline transitions to Error state after a failed transcription.
    assert_eq!(p.state(), PipelineState::Error);
}

// =============================================================================
// Mock Embeddings Provider for deterministic routing tests
// =============================================================================

use crate::embeddings::{EmbeddingsError, EmbeddingsProvider};
use serde_json::Value as JsonValue;
use std::collections::HashMap as StdHashMap;
use std::sync::Mutex;

/// A mock embeddings provider that returns deterministic embeddings based on text content.
///
/// This provider maps specific input texts to predetermined embedding vectors, enabling
/// deterministic offline tests for intent routing without making real API calls.
struct MockEmbeddingsProvider {
    /// Map from input text to embedding vector
    embeddings_map: Mutex<StdHashMap<String, Vec<f32>>>,
    /// Default embedding to return if text is not in map
    default_embedding: Vec<f32>,
    /// Optional error to simulate failures
    error: Option<String>,
}

impl MockEmbeddingsProvider {
    /// Create a new mock provider with a default embedding vector
    fn new(default_embedding: Vec<f32>) -> Self {
        Self {
            embeddings_map: Mutex::new(StdHashMap::new()),
            default_embedding,
            error: None,
        }
    }

    /// Add a text -> embedding mapping
    fn with_embedding(self, text: impl Into<String>, embedding: Vec<f32>) -> Self {
        self.embeddings_map
            .lock()
            .unwrap()
            .insert(text.into(), embedding);
        self
    }

    /// Configure the provider to return an error
    #[allow(dead_code)]
    fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }
}

#[async_trait]
impl EmbeddingsProvider for MockEmbeddingsProvider {
    async fn embed_text(
        &self,
        text: &str,
        _input_type: Option<&str>,
    ) -> Result<(Vec<f32>, JsonValue, JsonValue), EmbeddingsError> {
        if let Some(ref err) = self.error {
            return Err(EmbeddingsError::Api(err.clone()));
        }

        let embedding = self
            .embeddings_map
            .lock()
            .unwrap()
            .get(text)
            .cloned()
            .unwrap_or_else(|| self.default_embedding.clone());

        let request_json = serde_json::json!({
            "mock": true,
            "input": text,
        });
        let response_json = serde_json::json!({
            "mock": true,
            "embedding_len": embedding.len(),
        });

        Ok((embedding, request_json, response_json))
    }

    fn name(&self) -> &'static str {
        "mock"
    }

    fn model(&self) -> &str {
        "mock-embeddings-model"
    }
}

// =============================================================================
// Routing and Preset Selection Invariant Tests
// =============================================================================

use crate::llm::{ProgramPreset, ProgramPromptProfile, PromptSections};
use crate::settings::{IntentRouterSettings, IntentRouterStrategy};

/// Helper to create a profile with presets configured for embeddings routing
fn create_routing_test_profile(presets: Vec<(&str, &str, Vec<&str>)>) -> ProgramPromptProfile {
    let prompt_presets: Vec<ProgramPreset> = presets
        .into_iter()
        .map(|(id, name, hints)| ProgramPreset {
            model_selections: Default::default(),
            id: id.to_string(),
            name: name.to_string(),
            routing_hints: hints.into_iter().map(|s| s.to_string()).collect(),
            prompts: PromptSections::default(),
            rewrite_llm_enabled: true,
            stt_provider: None,
            stt_model: None,
            stt_language: None,
            stt_input_mode: None,
            stt_timeout_seconds: None,
            llm_provider: None,
            llm_model: None,
            openai_reasoning_effort: None,
            gemini_thinking_budget: None,
            gemini_thinking_level: None,
            anthropic_thinking_budget: None,
        })
        .collect();

    ProgramPromptProfile {
        model_selections: Default::default(),
        id: "test-profile".to_string(),
        name: "Test Profile".to_string(),
        program_paths: vec![],
        rewrite_llm_enabled: Some(true),
        rewrite_include_clipboard_context: None,
        stt_provider: None,
        stt_model: None,
        stt_language: None,
        stt_input_mode: None,
        stt_timeout_seconds: None,
        llm_provider: None,
        llm_model: None,
        openai_reasoning_effort: None,
        gemini_thinking_budget: None,
        gemini_thinking_level: None,
        anthropic_thinking_budget: None,
        prompts: PromptSections::default(),
        presets: prompt_presets,
        default_preset_id: None,
        default_preset_description: None,
        default_target_rewrite_llm_enabled: true,
        active_preset_id: None,
        router: Some(IntentRouterSettings {
            enabled: true,
            strategy: IntentRouterStrategy::Embeddings,
            embedding_provider: Some("mock".to_string()),
            embedding_model: Some("mock-model".to_string()),
            pick_highest_score: true, // Always pick the best match
            similarity_threshold: None,
            similarity_margin: None,
            llm_provider: None,
            llm_model: None,
            llm_system_prompt: None,
            openai_reasoning_effort: None,
            gemini_thinking_budget: None,
            gemini_thinking_level: None,
            anthropic_thinking_budget: None,
        }),
        quick_ask_provider: None,
        quick_ask_model: None,
        quick_ask_system_prompt: None,
        context_grab_method: None,
        quick_replace_include_clipboard_context: None,
        quick_ask_include_clipboard_context: None,
        quick_replace_enabled: None,
        quick_replace_provider: None,
        quick_replace_model: None,
        quick_replace_system_prompt: None,
        quick_ask_openai_reasoning_effort: None,
        quick_ask_gemini_thinking_budget: None,
        quick_ask_gemini_thinking_level: None,
        quick_ask_anthropic_thinking_budget: None,

        rewrite_active_window_ocr_mode: None,
        quick_replace_active_window_ocr_mode: None,
        quick_ask_active_window_ocr_mode: None,
    }
}

#[tokio::test]
async fn routing_selects_preset_with_highest_similarity() {
    // Given: A profile with two presets and embeddings configured so that
    // "send email" is very similar to the "email" preset hint.
    // Create embeddings where:
    // - Transcript "send email" -> [1.0, 0.0, 0.0]
    // - "email and messages" hint -> [0.95, 0.05, 0.0] (high similarity to transcript)
    // - "calendar events" hint -> [0.0, 1.0, 0.0] (low similarity)
    let mock_embeddings = Arc::new(
        MockEmbeddingsProvider::new(vec![0.5, 0.5, 0.0])
            .with_embedding("send email", vec![1.0, 0.0, 0.0])
            .with_embedding("email and messages", vec![0.95, 0.05, 0.0])
            .with_embedding("calendar events", vec![0.0, 1.0, 0.0]),
    );

    let profile = create_routing_test_profile(vec![
        ("email-preset", "Email", vec!["email and messages"]),
        ("calendar-preset", "Calendar", vec!["calendar events"]),
    ]);

    let mut config = test_config_for_transcription();
    config.llm_config = mock_llm_config(true, vec![profile.clone()]);
    insert_mock_llm_api_key(&mut config);

    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(MockSttProvider::new("send email")),
    );
    p.inject_llm_provider_for_tests(
        MOCK_PROVIDER,
        None,
        Arc::new(MockLlmProvider::new("Sending email")),
    );
    p.inject_embeddings_provider_for_tests(mock_embeddings);

    // Set the active profile
    {
        let mut inner = p.inner.lock().expect("pipeline lock");
        inner.config.llm_config.program_prompt_profiles = vec![profile];
    }

    p.start_recording().expect("start recording should succeed");

    // When: we stop and transcribe
    let result = p
        .stop_and_transcribe_detailed()
        .await
        .expect("stop/transcribe should succeed");

    // Then: the pipeline successfully completed (LLM rewrite happened)
    assert_eq!(result.final_text, "Sending email");
    assert_eq!(p.state(), PipelineState::Idle);
}

#[tokio::test]
async fn rewrite_consumes_manual_ocr_result_when_available() {
    // Given: manual OCR mode, and an OCR result already produced (e.g. user clicked
    // the overlay OCR pill while recording).
    let mut profile = create_routing_test_profile(vec![]);
    profile.id = "default".to_string();
    profile.name = "Default".to_string();
    // Keep this test focused: we only care about rewrite user-message assembly.
    profile.router = None;

    let mut config = test_config_for_transcription();
    config.llm_config = mock_llm_config(true, vec![profile.clone()]);
    insert_mock_llm_api_key(&mut config);
    config.ocr_config.rewrite_mode = "manual".to_string();

    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    // STT produces any transcript (not important).
    p.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(MockSttProvider::new("hello")),
    );
    // LLM returns a different string depending on whether OCR context made it into the prompt.
    p.inject_llm_provider_for_tests(
        MOCK_PROVIDER,
        None,
        Arc::new(ConditionalOcrLlmProvider::new("WITH_OCR", "NO_OCR")),
    );

    {
        let mut inner = p.inner.lock().expect("pipeline lock");
        inner.config.llm_config.program_prompt_profiles = vec![profile];
    }

    p.start_recording().expect("start recording should succeed");

    // Simulate: OCR already finished before we hit rewrite.
    {
        let mut inner = p.inner.lock().expect("pipeline lock");
        inner.ocr.result = Some(crate::ocr::OcrResult {
            text: "some ocr text".to_string(),
            provider: "mock".to_string(),
            model: "mock-model".to_string(),
        });
    }

    let result = p
        .stop_and_transcribe_detailed()
        .await
        .expect("stop/transcribe should succeed");

    assert_eq!(result.final_text, "WITH_OCR");
    assert_eq!(p.state(), PipelineState::Idle);

    // A saved dictation retry uses the same request-owned OCR result, without
    // starting capture or reading the user's actual screen.
    p.inner.lock().unwrap().ocr.result = Some(crate::ocr::OcrResult {
        text: "synthetic retry context".into(),
        provider: "mock".into(),
        model: "mock-model".into(),
    });
    let retried = p
        .transcribe_wav_bytes_detailed(vec![1, 2, 3])
        .await
        .unwrap();
    assert_eq!(retried.final_text, "WITH_OCR");
    assert_eq!(p.state(), PipelineState::Idle);
}

#[tokio::test]
async fn routing_with_no_matching_preset_uses_default() {
    // Given: A profile with presets but transcript doesn't match any well
    // Create embeddings where the transcript doesn't match any hints well
    let mock_embeddings = Arc::new(
        MockEmbeddingsProvider::new(vec![0.5, 0.5, 0.0])
            .with_embedding("random unrelated text", vec![0.0, 0.0, 1.0]) // orthogonal to hints
            .with_embedding("email and messages", vec![1.0, 0.0, 0.0])
            .with_embedding("calendar events", vec![0.0, 1.0, 0.0]),
    );

    let mut profile = create_routing_test_profile(vec![
        ("email-preset", "Email", vec!["email and messages"]),
        ("calendar-preset", "Calendar", vec!["calendar events"]),
    ]);
    // Set a default preset
    profile.default_preset_id = Some("email-preset".to_string());
    // Disable pick_highest_score to use threshold-based selection
    if let Some(ref mut router) = profile.router {
        router.pick_highest_score = false;
        router.similarity_threshold = Some(0.8); // High threshold
    }

    let mut config = test_config_for_transcription();
    config.llm_config = mock_llm_config(true, vec![profile.clone()]);
    insert_mock_llm_api_key(&mut config);

    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(MockSttProvider::new("random unrelated text")),
    );
    p.inject_llm_provider_for_tests(
        MOCK_PROVIDER,
        None,
        Arc::new(MockLlmProvider::new("Processed text")),
    );
    p.inject_embeddings_provider_for_tests(mock_embeddings);

    {
        let mut inner = p.inner.lock().expect("pipeline lock");
        inner.config.llm_config.program_prompt_profiles = vec![profile];
    }

    p.start_recording().expect("start recording should succeed");

    // When: we stop and transcribe
    let result = p
        .stop_and_transcribe_detailed()
        .await
        .expect("stop/transcribe should succeed");

    // Then: pipeline completes successfully using the default preset
    assert_eq!(result.final_text, "Processed text");
    assert_eq!(p.state(), PipelineState::Idle);
}

#[tokio::test]
async fn routing_respects_session_preset_lock() {
    // Given: A profile with routing enabled BUT a session preset lock is set
    let mock_embeddings = Arc::new(
        MockEmbeddingsProvider::new(vec![0.5, 0.5, 0.0])
            .with_embedding("send email", vec![1.0, 0.0, 0.0])
            .with_embedding("email and messages", vec![0.95, 0.05, 0.0])
            .with_embedding("calendar events", vec![0.0, 1.0, 0.0]),
    );

    let profile = create_routing_test_profile(vec![
        ("email-preset", "Email", vec!["email and messages"]),
        ("calendar-preset", "Calendar", vec!["calendar events"]),
    ]);

    let mut config = test_config_for_transcription();
    config.llm_config = mock_llm_config(true, vec![profile.clone()]);
    insert_mock_llm_api_key(&mut config);

    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(MockSttProvider::new("send email")), // Would normally route to email-preset
    );
    p.inject_llm_provider_for_tests(
        MOCK_PROVIDER,
        None,
        Arc::new(MockLlmProvider::new("Calendar event created")),
    );
    p.inject_embeddings_provider_for_tests(mock_embeddings);

    {
        let mut inner = p.inner.lock().expect("pipeline lock");
        inner.config.llm_config.program_prompt_profiles = vec![profile];
    }

    // Set session lock to force calendar-preset (overriding what routing would choose)
    p.set_session_preset_lock(None, Some("calendar-preset".to_string()))
        .expect("set session lock");

    p.start_recording().expect("start recording should succeed");

    // When: we stop and transcribe
    let result = p
        .stop_and_transcribe_detailed()
        .await
        .expect("stop/transcribe should succeed");

    // Then: session lock was respected (we got calendar output despite email-like input)
    assert_eq!(result.final_text, "Calendar event created");
    assert_eq!(p.state(), PipelineState::Idle);
}

#[tokio::test]
async fn embeddings_provider_error_gracefully_falls_back() {
    // Given: An embeddings provider configured to fail
    let mock_embeddings = Arc::new(
        MockEmbeddingsProvider::new(vec![0.5, 0.5, 0.0])
            .with_error("Simulated embeddings API failure"),
    );

    let profile =
        create_routing_test_profile(vec![("email-preset", "Email", vec!["email and messages"])]);

    let mut config = test_config_for_transcription();
    config.llm_config = mock_llm_config(true, vec![profile.clone()]);
    insert_mock_llm_api_key(&mut config);

    let p = SharedPipeline::new_for_tests(config, Box::new(FakeAudioCapture::new()));
    p.inject_stt_provider_for_tests(
        MOCK_PROVIDER,
        None,
        None,
        Arc::new(MockSttProvider::new("send email")),
    );
    p.inject_llm_provider_for_tests(
        MOCK_PROVIDER,
        None,
        Arc::new(MockLlmProvider::new("Fallback output")),
    );
    p.inject_embeddings_provider_for_tests(mock_embeddings);

    {
        let mut inner = p.inner.lock().expect("pipeline lock");
        inner.config.llm_config.program_prompt_profiles = vec![profile];
    }

    p.start_recording().expect("start recording should succeed");

    // When: we stop and transcribe (embeddings will fail)
    let result = p
        .stop_and_transcribe_detailed()
        .await
        .expect("stop/transcribe should succeed even with embeddings failure");

    // Then: pipeline completes successfully (routing failed but LLM rewrite still worked)
    assert_eq!(result.final_text, "Fallback output");
    assert_eq!(p.state(), PipelineState::Idle);
}
