use super::*;
use crate::embeddings::EmbeddingsError;
use crate::llm::{LlmError, ProgramPreset, ProgramPromptProfile, PromptSections};
use crate::settings::IntentRouterSettings;
use async_trait::async_trait;
use std::collections::HashMap;

struct FakeEmbeddingsProvider {
    values: HashMap<String, Vec<f32>>,
    error_for: Option<String>,
}

impl FakeEmbeddingsProvider {
    fn new(values: &[(&str, Vec<f32>)]) -> Self {
        Self {
            values: values
                .iter()
                .map(|(key, value)| ((*key).to_string(), value.clone()))
                .collect(),
            error_for: None,
        }
    }

    fn with_error_for(mut self, text: &str) -> Self {
        self.error_for = Some(text.to_string());
        self
    }
}

#[async_trait]
impl EmbeddingsProvider for FakeEmbeddingsProvider {
    async fn embed_text(
        &self,
        text: &str,
        input_type: Option<&str>,
    ) -> Result<(Vec<f32>, JsonValue, JsonValue), EmbeddingsError> {
        if self.error_for.as_deref() == Some(text) {
            return Err(EmbeddingsError::Api("planned failure".to_string()));
        }
        let embedding = self
            .values
            .get(text)
            .cloned()
            .unwrap_or_else(|| vec![0.0, 0.0]);
        Ok((
            embedding.clone(),
            serde_json::json!({ "text": text, "input_type": input_type }),
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

struct FakeLlmProvider {
    value: Result<JsonValue, String>,
}

#[async_trait]
impl LlmProvider for FakeLlmProvider {
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
    ) -> Result<String, LlmError> {
        Ok("{}".to_string())
    }

    async fn complete_json_schema(
        &self,
        _system_prompt: &str,
        _user_message: &str,
        _schema_name: &str,
        _schema_description: &str,
        _schema: JsonValue,
    ) -> Result<JsonValue, LlmError> {
        self.value
            .clone()
            .map_err(|error| LlmError::Api(error.to_string()))
    }

    fn name(&self) -> &'static str {
        "fake"
    }

    fn model(&self) -> &str {
        "fake-llm-model"
    }
}

fn preset(id: &str, hint: &str) -> ProgramPreset {
    ProgramPreset {
        model_selections: Default::default(),
        id: id.to_string(),
        name: id.to_string(),
        routing_hints: vec![hint.to_string()],
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
    }
}

fn profile(strategy: IntentRouterStrategy) -> ProgramPromptProfile {
    ProgramPromptProfile {
        model_selections: Default::default(),
        id: "profile".to_string(),
        name: "Profile".to_string(),
        program_paths: vec![],
        prompts: PromptSections::default(),
        presets: vec![
            preset("email", "email hint"),
            preset("calendar", "calendar hint"),
        ],
        default_preset_id: None,
        default_preset_description: Some("default target".to_string()),
        default_target_rewrite_llm_enabled: true,
        active_preset_id: None,
        router: Some(IntentRouterSettings {
            enabled: true,
            strategy,
            embedding_provider: Some("openai".to_string()),
            embedding_model: Some("fake-embedding-model".to_string()),
            pick_highest_score: false,
            similarity_threshold: Some(0.75),
            similarity_margin: Some(0.10),
            llm_provider: Some("fake".to_string()),
            llm_model: Some("fake-llm-model".to_string()),
            llm_system_prompt: None,
            openai_reasoning_effort: None,
            gemini_thinking_budget: None,
            gemini_thinking_level: None,
            anthropic_thinking_budget: None,
        }),
        rewrite_llm_enabled: Some(true),
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

async fn embeddings_decision(
    provider: FakeEmbeddingsProvider,
    profile: &ProgramPromptProfile,
    transcript: &str,
) -> RoutingDecision {
    route_preset_id_with_embeddings(
        profile,
        transcript,
        &ProxySettings::default(),
        &HashMap::new(),
        &Arc::new(Mutex::new(HashMap::new())),
        None,
        Some(Arc::new(provider)),
    )
    .await
    .expect("router should be attempted")
}

#[tokio::test]
async fn embeddings_selected_preset_default_no_decision_and_ambiguity_are_distinct() {
    let profile = profile(IntentRouterStrategy::Embeddings);

    let selected = embeddings_decision(
        FakeEmbeddingsProvider::new(&[
            ("send email", vec![1.0, 0.0]),
            ("email hint", vec![0.95, 0.05]),
            ("calendar hint", vec![0.0, 1.0]),
            ("default target", vec![0.0, 1.0]),
        ]),
        &profile,
        "send email",
    )
    .await;
    assert_eq!(selected.outcome, RoutingDecisionOutcome::SelectedPreset);
    assert_eq!(selected.selected_preset_id.as_deref(), Some("email"));

    let default = embeddings_decision(
        FakeEmbeddingsProvider::new(&[
            ("general words", vec![1.0, 0.0]),
            ("email hint", vec![0.0, 1.0]),
            ("calendar hint", vec![0.0, 1.0]),
            ("default target", vec![0.95, 0.05]),
        ]),
        &profile,
        "general words",
    )
    .await;
    assert_eq!(default.outcome, RoutingDecisionOutcome::DefaultTarget);
    assert_eq!(default.selected_preset_id, None);

    let no_decision = embeddings_decision(
        FakeEmbeddingsProvider::new(&[
            ("unclear", vec![1.0, 0.0]),
            ("email hint", vec![0.1, 0.9]),
            ("calendar hint", vec![0.0, 1.0]),
            ("default target", vec![0.0, 1.0]),
        ]),
        &profile,
        "unclear",
    )
    .await;
    assert_eq!(no_decision.outcome, RoutingDecisionOutcome::NoDecision);

    let ambiguous = embeddings_decision(
        FakeEmbeddingsProvider::new(&[
            ("message", vec![1.0, 0.0]),
            ("email hint", vec![0.95, 0.05]),
            ("calendar hint", vec![0.90, 0.10]),
            ("default target", vec![0.0, 1.0]),
        ]),
        &profile,
        "message",
    )
    .await;
    assert_eq!(ambiguous.outcome, RoutingDecisionOutcome::Ambiguous);
}

#[tokio::test]
async fn embeddings_failure_records_failed_decision_diagnostics() {
    let profile = profile(IntentRouterStrategy::Embeddings);
    let decision = embeddings_decision(
        FakeEmbeddingsProvider::new(&[]).with_error_for("boom"),
        &profile,
        "boom",
    )
    .await;

    assert_eq!(decision.outcome, RoutingDecisionOutcome::Failed);
    assert_eq!(decision.selected_preset_id, None);
    assert_eq!(decision.response_json["outcome"], "failed");
}

#[tokio::test]
async fn embeddings_router_uses_shared_cache_key_and_input_roles() {
    let mut profile = profile(IntentRouterStrategy::Embeddings);
    if let Some(router) = profile.router.as_mut() {
        router.embedding_provider = Some("cohere".to_string());
        router.embedding_model = Some("embed-english-v3.0".to_string());
    }

    let cache = Arc::new(Mutex::new(HashMap::new()));
    let decision = route_preset_id_with_embeddings(
        &profile,
        "send email",
        &ProxySettings::default(),
        &HashMap::new(),
        &cache,
        None,
        Some(Arc::new(FakeEmbeddingsProvider::new(&[
            ("send email", vec![1.0, 0.0]),
            ("email hint", vec![0.95, 0.05]),
            ("calendar hint", vec![0.0, 1.0]),
            ("default target", vec![0.0, 1.0]),
        ]))),
    )
    .await
    .expect("router should be attempted");

    let expected_key = crate::router_embeddings_cache::router_embedding_cache_key(
        "cohere",
        "embed-english-v3.0",
        "email hint",
    );
    assert!(
        cache
            .lock()
            .expect("cache lock")
            .contains_key(&expected_key),
        "hint embeddings should be stored under the shared cache-key helper"
    );

    let calls = decision.request_json["calls"]
        .as_array()
        .expect("request calls array");
    let email_hint_call = calls
        .iter()
        .find(|call| {
            call["kind"] == "hint"
                && call["candidate_id"] == "email"
                && call["request"]["text"] == "email hint"
        })
        .expect("email hint call");
    assert_eq!(email_hint_call["request"]["input_type"], "search_document");
}

#[tokio::test]
async fn llm_selected_default_failed_and_unknown_outcomes_are_distinct() {
    let profile = profile(IntentRouterStrategy::Llm);

    let selected = route_preset_id_with_llm(
        &profile,
        "send email",
        &FakeLlmProvider {
            value: Ok(serde_json::json!({ "preset_id": "email" })),
        },
    )
    .await
    .expect("router should be attempted");
    assert_eq!(selected.outcome, RoutingDecisionOutcome::SelectedPreset);
    assert_eq!(selected.selected_preset_id.as_deref(), Some("email"));

    let default = route_preset_id_with_llm(
        &profile,
        "use default",
        &FakeLlmProvider {
            value: Ok(serde_json::json!({ "preset_id": "default" })),
        },
    )
    .await
    .expect("router should be attempted");
    assert_eq!(default.outcome, RoutingDecisionOutcome::DefaultTarget);
    assert_eq!(default.selected_preset_id, None);

    let failed = route_preset_id_with_llm(
        &profile,
        "fail",
        &FakeLlmProvider {
            value: Err("planned failure".to_string()),
        },
    )
    .await
    .expect("router should be attempted");
    assert_eq!(failed.outcome, RoutingDecisionOutcome::Failed);
    assert_eq!(failed.response_json["outcome"], "failed");

    let unknown = route_preset_id_with_llm(
        &profile,
        "unknown",
        &FakeLlmProvider {
            value: Ok(serde_json::json!({ "preset_id": "bogus" })),
        },
    )
    .await
    .expect("router should be attempted");
    assert_eq!(unknown.outcome, RoutingDecisionOutcome::UnknownPreset);
    assert_eq!(unknown.selected_preset_id, None);
}
