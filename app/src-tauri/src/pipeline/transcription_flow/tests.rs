use super::*;
use crate::embeddings::EmbeddingsError;
use crate::llm::{LlmError, ProgramPromptProfile, PromptSections};
use crate::pipeline::llm_provider::LlmProviderParams;
use crate::settings::IntentRouterSettings;
use async_trait::async_trait;

struct RecordingCallbacks(std::sync::Mutex<Option<(String, Option<String>)>>);

impl TranscriptionCallbacks for RecordingCallbacks {
    fn transition_to_routing(&self) {}
    fn transition_from_routing(&self) {}
    fn transition_to_rewriting(&self) {}

    fn get_or_create_llm_provider(
        &self,
        provider_id: &str,
        params: LlmProviderParams,
    ) -> Result<std::sync::Arc<dyn crate::llm::LlmProvider>, crate::pipeline::PipelineError> {
        *self.0.lock().expect("lock") = Some((provider_id.to_string(), params.model.clone()));

        // We don't need a real provider instance for this test; we only validate selection.
        Err(crate::pipeline::PipelineError::Config(
            "test: provider creation not needed".to_string(),
        ))
    }
}

struct RoutingCallbacks {
    transitions: std::sync::Mutex<Vec<&'static str>>,
}

impl RoutingCallbacks {
    fn new() -> Self {
        Self {
            transitions: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl TranscriptionCallbacks for RoutingCallbacks {
    fn transition_to_routing(&self) {
        self.transitions.lock().expect("lock").push("to_routing");
    }

    fn transition_from_routing(&self) {
        self.transitions.lock().expect("lock").push("from_routing");
    }

    fn transition_to_rewriting(&self) {}

    fn get_or_create_llm_provider(
        &self,
        _provider_id: &str,
        _params: LlmProviderParams,
    ) -> Result<std::sync::Arc<dyn crate::llm::LlmProvider>, crate::pipeline::PipelineError> {
        Err(crate::pipeline::PipelineError::Config(
            "test: provider creation not needed".to_string(),
        ))
    }
}

struct FlowEmbeddingsProvider(std::collections::HashMap<String, Vec<f32>>);

#[async_trait]
impl crate::embeddings::EmbeddingsProvider for FlowEmbeddingsProvider {
    async fn embed_text(
        &self,
        text: &str,
        _input_type: Option<&str>,
    ) -> Result<(Vec<f32>, serde_json::Value, serde_json::Value), EmbeddingsError> {
        let embedding = self.0.get(text).cloned().unwrap_or_else(|| vec![0.0, 0.0]);
        Ok((
            embedding.clone(),
            serde_json::json!({ "text": text }),
            serde_json::json!({ "embedding_len": embedding.len() }),
        ))
    }

    fn name(&self) -> &'static str {
        "openai"
    }

    fn model(&self) -> &str {
        "fake-embedding-model"
    }
}

struct PanicLlmProvider;

struct FlowLlmProvider {
    cancel: Option<CancellationToken>,
}

#[async_trait]
impl LlmProvider for FlowLlmProvider {
    async fn complete(&self, _system: &str, user: &str) -> Result<String, LlmError> {
        assert!(user.contains("raw transcript"));
        Ok("corrected transcript".into())
    }

    async fn complete_json_schema(
        &self,
        _system: &str,
        user: &str,
        name: &str,
        _description: &str,
        schema: serde_json::Value,
    ) -> Result<serde_json::Value, LlmError> {
        assert!(user.contains("raw transcript"));
        assert_eq!(name, "intent_router_choice");
        assert!(schema["properties"]["preset_id"]["enum"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("email")));
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
        Ok(serde_json::json!({"preset_id": "email"}))
    }

    fn name(&self) -> &'static str {
        "fake"
    }
    fn model(&self) -> &str {
        "fake-model"
    }
}

struct ProviderCallbacks {
    transitions: Mutex<Vec<&'static str>>,
    provider: Arc<dyn LlmProvider>,
}

impl TranscriptionCallbacks for ProviderCallbacks {
    fn transition_to_routing(&self) {
        self.transitions.lock().unwrap().push("routing");
    }
    fn transition_from_routing(&self) {
        self.transitions.lock().unwrap().push("transcribing");
    }
    fn transition_to_rewriting(&self) {
        self.transitions.lock().unwrap().push("rewriting");
    }
    fn get_or_create_llm_provider(
        &self,
        _: &str,
        _: LlmProviderParams,
    ) -> Result<Arc<dyn LlmProvider>, PipelineError> {
        Ok(self.provider.clone())
    }
}

fn logging_context(cache: &Arc<Mutex<HashMap<String, Vec<f32>>>>) -> TranscriptionContext<'_> {
    let mut profile = embeddings_router_profile();
    profile.router.as_mut().unwrap().strategy = IntentRouterStrategy::Llm;
    profile.default_preset_description = Some("Other requests".into());
    let logs = RequestLogStore::new();
    logs.start_request("fake-stt".into(), None);
    TranscriptionContext {
        active_profile: Some(profile),
        active_window_ocr_text: Some("synthetic context".into()),
        llm_enabled_global: true,
        default_rewrite_include_clipboard_context: false,
        session_lock: None,
        proxy_settings: ProxySettings::default(),
        llm_api_keys: HashMap::new(),
        request_log_store: Some(logs),
        embedding_cache: cache,
        persist_app: None,
        cancel_token: CancellationToken::new(),
        injected_embeddings_provider: None,
        force_llm_rewrite: false,
        forced_llm_provider: None,
        forced_llm_model: None,
    }
}

#[tokio::test]
async fn llm_flow_records_selected_preset_and_actual_rewrite_provider() {
    let cache = Arc::new(Mutex::new(HashMap::new()));
    let ctx = logging_context(&cache);
    let callbacks = ProviderCallbacks {
        transitions: Mutex::new(Vec::new()),
        provider: Arc::new(FlowLlmProvider { cancel: None }),
    };
    let result = complete_transcription_flow(
        &ctx,
        &callbacks,
        "raw transcript",
        42,
        None,
        &crate::llm::LlmConfig::default(),
    )
    .await
    .unwrap();
    assert_eq!(result.final_text, "corrected transcript");
    assert_eq!(result.stt_text, "raw transcript");
    assert!(matches!(result.llm_outcome, LlmOutcome::Succeeded));
    assert_eq!(
        *callbacks.transitions.lock().unwrap(),
        ["routing", "transcribing", "rewriting"]
    );
    let log = ctx
        .request_log_store
        .as_ref()
        .unwrap()
        .get_logs(Some(1))
        .pop()
        .unwrap();
    assert_eq!(log.preset_id.as_deref(), Some("email"));
    assert_eq!(log.llm_provider.as_deref(), Some("fake"));
    assert_eq!(log.llm_model.as_deref(), Some("fake-model"));
    assert_eq!(log.router_strategy.as_deref(), Some("llm"));
    let scores = log.router_scores.unwrap();
    assert_eq!(scores.len(), 3);
    assert!(scores
        .iter()
        .any(|score| score.preset_id == "email" && score.selected));
    assert!(scores
        .iter()
        .any(|score| score.preset_id == "__default__" && !score.selected));
    assert!(log.ocr_context_present);
    assert_eq!(log.ocr_context_chars, Some(17));
    assert!(log.rewrite_clipboard_context.is_none());
}

#[tokio::test]
async fn unavailable_rewrite_preserves_transcript_and_records_reason() {
    let cache = Arc::new(Mutex::new(HashMap::new()));
    let mut ctx = logging_context(&cache);
    ctx.active_profile.as_mut().unwrap().active_preset_id = Some("email".into());
    let callbacks = RecordingCallbacks(Mutex::new(None));
    let result = complete_transcription_flow(
        &ctx,
        &callbacks,
        "raw transcript",
        42,
        None,
        &crate::llm::LlmConfig::default(),
    )
    .await
    .unwrap();
    assert_eq!(result.final_text, "raw transcript");
    assert!(matches!(
        result.llm_outcome,
        LlmOutcome::NotAttempted(LlmNotAttemptedReason::ProviderUnavailable { .. })
    ));
    let log = ctx
        .request_log_store
        .as_ref()
        .unwrap()
        .get_logs(Some(1))
        .pop()
        .unwrap();
    assert_eq!(log.preset_id.as_deref(), Some("email"));
    assert!(log
        .entries
        .iter()
        .any(|entry| entry.level == crate::request_log::LogLevel::Warn
            && entry.message.contains("unavailable")));
}

#[tokio::test]
async fn routing_cancellation_never_rewrites_or_overwrites_the_new_log() {
    let cache = Arc::new(Mutex::new(HashMap::new()));
    let ctx = logging_context(&cache);
    let callbacks = ProviderCallbacks {
        transitions: Mutex::new(Vec::new()),
        provider: Arc::new(FlowLlmProvider {
            cancel: Some(ctx.cancel_token.clone()),
        }),
    };
    let result = complete_transcription_flow(
        &ctx,
        &callbacks,
        "raw transcript",
        42,
        None,
        &crate::llm::LlmConfig::default(),
    )
    .await;
    assert!(matches!(result, Err(PipelineError::Cancelled)));
    assert!(!callbacks.transitions.lock().unwrap().contains(&"rewriting"));
    let log = ctx
        .request_log_store
        .as_ref()
        .unwrap()
        .get_logs(Some(1))
        .pop()
        .unwrap();
    assert!(log.router_response_json.is_none());
    assert!(log.preset_id.is_none());
    // Already-cancelled routing must not even poll the provider.
    let callbacks = ProviderCallbacks {
        transitions: Mutex::new(Vec::new()),
        provider: Arc::new(PanicLlmProvider),
    };
    assert!(run_llm_router(
        &ctx,
        &callbacks,
        ctx.active_profile.as_ref().unwrap(),
        "raw transcript"
    )
    .await
    .is_none());
}

#[async_trait]
impl crate::llm::LlmProvider for PanicLlmProvider {
    async fn complete(
        &self,
        _system_prompt: &str,
        _user_message: &str,
    ) -> Result<String, LlmError> {
        panic!("cancelled rewrite should not call provider");
    }

    fn name(&self) -> &'static str {
        "panic"
    }

    fn model(&self) -> &str {
        "panic-model"
    }
}

fn minimal_profile_with_llm_overrides(
    provider: Option<&str>,
    model: Option<&str>,
) -> ProgramPromptProfile {
    ProgramPromptProfile {
        id: "test-profile".to_string(),
        name: "Test Profile".to_string(),
        program_paths: vec![],
        prompts: PromptSections::default(),
        presets: vec![],
        default_preset_id: None,
        default_preset_description: None,
        default_target_rewrite_llm_enabled: true,
        active_preset_id: None,
        router: None,
        rewrite_llm_enabled: Some(true),
        stt_provider: None,
        stt_model: None,
        stt_language: None,
        stt_timeout_seconds: None,
        llm_provider: provider.map(|s| s.to_string()),
        llm_model: model.map(|s| s.to_string()),
        openai_reasoning_effort: None,
        gemini_thinking_budget: None,
        gemini_thinking_level: None,
        anthropic_thinking_budget: None,
        quick_ask_provider: None,
        quick_ask_model: None,
        quick_ask_system_prompt: None,
        context_grab_method: None,
        rewrite_include_clipboard_context: None,
        quick_replace_include_clipboard_context: None,
        quick_ask_include_clipboard_context: None,
        rewrite_active_window_ocr_mode: None,
        quick_replace_active_window_ocr_mode: None,
        quick_ask_active_window_ocr_mode: None,
        quick_replace_enabled: None,
        quick_replace_provider: None,
        quick_replace_model: None,
        quick_replace_system_prompt: None,
        quick_ask_openai_reasoning_effort: None,
        quick_ask_gemini_thinking_budget: None,
        quick_ask_gemini_thinking_level: None,
        quick_ask_anthropic_thinking_budget: None,
    }
}

fn embeddings_router_profile() -> ProgramPromptProfile {
    let mut profile = minimal_profile_with_llm_overrides(None, None);
    profile.presets = vec![
        crate::llm::ProgramPreset {
            id: "email".to_string(),
            name: "Email".to_string(),
            routing_hints: vec!["email hint".to_string()],
            prompts: PromptSections::default(),
            rewrite_llm_enabled: true,
            stt_provider: None,
            stt_model: None,
            stt_language: None,
            stt_timeout_seconds: None,
            llm_provider: None,
            llm_model: None,
            openai_reasoning_effort: None,
            gemini_thinking_budget: None,
            gemini_thinking_level: None,
            anthropic_thinking_budget: None,
        },
        crate::llm::ProgramPreset {
            id: "calendar".to_string(),
            name: "Calendar".to_string(),
            routing_hints: vec!["calendar hint".to_string()],
            prompts: PromptSections::default(),
            rewrite_llm_enabled: true,
            stt_provider: None,
            stt_model: None,
            stt_language: None,
            stt_timeout_seconds: None,
            llm_provider: None,
            llm_model: None,
            openai_reasoning_effort: None,
            gemini_thinking_budget: None,
            gemini_thinking_level: None,
            anthropic_thinking_budget: None,
        },
    ];
    profile.router = Some(IntentRouterSettings {
        enabled: true,
        strategy: IntentRouterStrategy::Embeddings,
        embedding_provider: Some("openai".to_string()),
        embedding_model: Some("fake-embedding-model".to_string()),
        pick_highest_score: false,
        similarity_threshold: Some(0.75),
        similarity_margin: Some(0.10),
        llm_provider: None,
        llm_model: None,
        llm_system_prompt: None,
        openai_reasoning_effort: None,
        gemini_thinking_budget: None,
        gemini_thinking_level: None,
        anthropic_thinking_budget: None,
    });
    profile
}

#[test]
fn forced_llm_provider_model_take_precedence_over_profile() {
    let profile = minimal_profile_with_llm_overrides(Some("ollama"), Some("some-model"));
    let callbacks = RecordingCallbacks(std::sync::Mutex::new(None));

    let embedding_cache: std::sync::Arc<
        std::sync::Mutex<std::collections::HashMap<String, Vec<f32>>>,
    > = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));

    let ctx = TranscriptionContext {
        active_profile: Some(profile),
        active_window_ocr_text: None,
        llm_enabled_global: true,
        default_rewrite_include_clipboard_context: false,
        session_lock: None,
        proxy_settings: crate::settings::ProxySettings::default(),
        llm_api_keys: std::collections::HashMap::new(),
        request_log_store: None,
        embedding_cache: &embedding_cache,
        persist_app: None,
        cancel_token: tokio_util::sync::CancellationToken::new(),
        injected_embeddings_provider: None,
        force_llm_rewrite: true,
        forced_llm_provider: Some("groq".to_string()),
        forced_llm_model: Some("llama-3.1-8b-instant".to_string()),
    };

    let llm_config = crate::llm::LlmConfig::default();
    let _ = resolve_llm_for_rewrite(&ctx, &callbacks, &None, &llm_config);

    let recorded = callbacks.0.lock().expect("lock").clone();
    assert_eq!(
        recorded,
        Some(("groq".to_string(), Some("llama-3.1-8b-instant".to_string())))
    );
}

#[tokio::test]
async fn route_preset_consumes_routing_decision_and_records_outcome() {
    let profile = embeddings_router_profile();
    let callbacks = RoutingCallbacks::new();
    let request_log_store = RequestLogStore::new();
    request_log_store.start_request("mock-stt".to_string(), None);
    let embedding_cache: std::sync::Arc<
        std::sync::Mutex<std::collections::HashMap<String, Vec<f32>>>,
    > = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    let provider = std::sync::Arc::new(FlowEmbeddingsProvider(
        [
            ("send email".to_string(), vec![1.0, 0.0]),
            ("email hint".to_string(), vec![0.95, 0.05]),
            ("calendar hint".to_string(), vec![0.0, 1.0]),
        ]
        .into_iter()
        .collect(),
    ));

    let ctx = TranscriptionContext {
        active_profile: Some(profile),
        active_window_ocr_text: None,
        llm_enabled_global: true,
        default_rewrite_include_clipboard_context: false,
        session_lock: None,
        proxy_settings: crate::settings::ProxySettings::default(),
        llm_api_keys: std::collections::HashMap::new(),
        request_log_store: Some(request_log_store.clone()),
        embedding_cache: &embedding_cache,
        persist_app: None,
        cancel_token: tokio_util::sync::CancellationToken::new(),
        injected_embeddings_provider: Some(provider),
        force_llm_rewrite: false,
        forced_llm_provider: None,
        forced_llm_model: None,
    };

    let result = route_preset(&ctx, &callbacks, "send email").await;

    assert_eq!(result.routed_preset_id.as_deref(), Some("email"));
    assert_eq!(
        callbacks.transitions.lock().expect("lock").as_slice(),
        ["to_routing", "from_routing"]
    );
    let response = request_log_store
        .with_current(|log| log.router_response_json.clone())
        .flatten()
        .expect("router response should be recorded");
    assert_eq!(response["outcome"], "selected_preset");
    assert_eq!(response["type"], "embeddings");
}

#[tokio::test]
async fn cancellation_outranks_provider_work_in_rewrite_step() {
    let profile = minimal_profile_with_llm_overrides(None, None);
    let callbacks = RecordingCallbacks(std::sync::Mutex::new(None));
    let embedding_cache: std::sync::Arc<
        std::sync::Mutex<std::collections::HashMap<String, Vec<f32>>>,
    > = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    let cancel_token = tokio_util::sync::CancellationToken::new();
    cancel_token.cancel();
    let logs = RequestLogStore::new();
    let new_id = logs.start_request("new-session".into(), None);
    logs.with_current(|log| {
        log.preset_id = Some("new-preset".into());
        log.llm_provider = Some("new-provider".into());
    });
    let ctx = TranscriptionContext {
        active_profile: Some(profile),
        active_window_ocr_text: None,
        llm_enabled_global: true,
        default_rewrite_include_clipboard_context: false,
        session_lock: None,
        proxy_settings: crate::settings::ProxySettings::default(),
        llm_api_keys: std::collections::HashMap::new(),
        request_log_store: Some(logs.clone()),
        embedding_cache: &embedding_cache,
        persist_app: None,
        cancel_token,
        injected_embeddings_provider: None,
        force_llm_rewrite: true,
        forced_llm_provider: None,
        forced_llm_model: None,
    };

    let resolution = LlmResolution {
        provider: Some(std::sync::Arc::new(PanicLlmProvider)),
        prompts: PromptSections::default(),
        timeout: Duration::from_secs(30),
        not_attempted_reason: None,
    };

    let (final_text, duration, outcome, provider, model) =
        run_llm_rewrite(&ctx, &callbacks, "raw transcript", resolution).await;

    assert_eq!(final_text, "raw transcript");
    assert!(duration.is_some());
    assert!(matches!(
        outcome,
        LlmOutcome::NotAttempted(LlmNotAttemptedReason::Unknown)
    ));
    assert_eq!(provider.as_deref(), Some("panic"));
    assert_eq!(model.as_deref(), Some("panic-model"));
    log_preset_selection(&ctx, &Some("old-preset".into()));
    let current = logs.get_logs(Some(1)).pop().unwrap();
    assert_eq!(current.id, new_id);
    assert_eq!(current.preset_id.as_deref(), Some("new-preset"));
    assert_eq!(current.llm_provider.as_deref(), Some("new-provider"));
    assert!(matches!(
        complete_transcription_flow(&ctx, &callbacks, "cancelled", 1, None, &Default::default())
            .await,
        Err(PipelineError::Cancelled)
    ));
}
