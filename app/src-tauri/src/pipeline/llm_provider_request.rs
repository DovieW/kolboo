//! One-off LLM provider request resolution for command and quick-action flows.
//!
//! The cached runtime path in `llm_provider.rs` already owns provider construction and cache
//! identity. This module stays narrower: it turns layered one-off inputs (global defaults,
//! optional profile overrides, and per-invocation overrides) into the concrete provider id +
//! `LlmProviderParams` bundle that the existing constructors already understand.
//!
//! Keeping this precedence logic local prevents Prompt Lab, History analysis, Quick Ask, and
//! Quick Replace from each growing their own slightly-different fallback ladder the next time we
//! add a provider-specific knob.

use std::collections::HashMap;
use std::sync::Arc;

use crate::llm::{LlmConfig, LlmProvider, ProgramPromptProfile};
use crate::pipeline::llm_provider::{
    create_one_off_llm_provider_unstructured, create_one_off_llm_provider_without_timeout,
    LlmProviderParams,
};
use crate::pipeline::PipelineError;
use crate::request_log::RequestLogStore;

/// One precedence layer in a one-off provider request.
///
/// Later layers win over earlier layers, but empty/whitespace-only strings never clobber an
/// existing value. That keeps UI helpers free to pass optional text boxes straight through
/// without accidentally erasing a profile/global choice with `"   "`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct OneOffLlmProviderRequestLayer {
    pub(crate) selection: Option<crate::model_selection::ModelSelection>,
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) ollama_url: Option<String>,
    pub(crate) openai_reasoning_effort: Option<String>,
    pub(crate) gemini_thinking_budget: Option<i64>,
    pub(crate) gemini_thinking_level: Option<String>,
    pub(crate) anthropic_thinking_budget: Option<i64>,
}

impl OneOffLlmProviderRequestLayer {
    pub(crate) fn from_preset(
        preset: Option<&crate::llm::ProgramPreset>,
        action: crate::model_selection::ModelAction,
    ) -> Self {
        let Some(preset) = preset else {
            return Self::default();
        };
        Self {
            selection: preset.model_selections.get(&action).cloned(),
            provider: (action == crate::model_selection::ModelAction::Rewrite)
                .then_some(preset.llm_provider.clone())
                .flatten(),
            model: (action == crate::model_selection::ModelAction::Rewrite)
                .then_some(preset.llm_model.clone())
                .flatten(),
            openai_reasoning_effort: preset.openai_reasoning_effort.clone(),
            gemini_thinking_budget: preset.gemini_thinking_budget,
            gemini_thinking_level: preset.gemini_thinking_level.clone(),
            anthropic_thinking_budget: preset.anthropic_thinking_budget,
            ..Default::default()
        }
    }
    /// Convert the rewrite-profile portion of persisted settings into a one-off resolution layer.
    ///
    /// Quick Ask / Quick Replace keep their own product-specific config Modules. They should map
    /// into `OneOffLlmProviderRequestLayer` at the callsite instead of teaching this pipeline
    /// Module about every quick-action read model.
    pub(crate) fn from_profile(profile: Option<&ProgramPromptProfile>) -> Self {
        let Some(profile) = profile else {
            return Self::default();
        };

        Self {
            selection: profile
                .model_selections
                .get(&crate::model_selection::ModelAction::Rewrite)
                .cloned(),
            provider: profile.llm_provider.clone(),
            model: profile.llm_model.clone(),
            ollama_url: None,
            openai_reasoning_effort: profile.openai_reasoning_effort.clone(),
            gemini_thinking_budget: profile.gemini_thinking_budget,
            gemini_thinking_level: profile.gemini_thinking_level.clone(),
            anthropic_thinking_budget: profile.anthropic_thinking_budget,
        }
    }
}

/// The normalized provider request used by one-off command and quick-action paths.
#[derive(Debug, Clone)]
pub(crate) struct OneOffLlmProviderRequest {
    pub(crate) provider_id: String,
    pub(crate) params: LlmProviderParams,
}

impl OneOffLlmProviderRequest {
    /// Use the existing unstructured provider constructor with a pre-resolved request.
    pub(crate) fn create_unstructured(
        &self,
        base_config: &LlmConfig,
        llm_api_keys: &HashMap<String, String>,
        proxy_settings: &crate::settings::ProxySettings,
    ) -> Result<Arc<dyn LlmProvider>, PipelineError> {
        create_one_off_llm_provider_unstructured(
            base_config,
            llm_api_keys,
            self.provider_id.as_str(),
            self.params.clone(),
            proxy_settings,
        )
    }

    /// Use the existing no-timeout provider constructor with a pre-resolved request.
    pub(crate) fn create_without_timeout(
        &self,
        base_config: &LlmConfig,
        llm_api_keys: &HashMap<String, String>,
        proxy_settings: &crate::settings::ProxySettings,
        request_log_store: Option<RequestLogStore>,
    ) -> Result<Arc<dyn LlmProvider>, PipelineError> {
        create_one_off_llm_provider_without_timeout(
            base_config,
            llm_api_keys,
            self.provider_id.as_str(),
            self.params.clone(),
            proxy_settings,
            request_log_store,
        )
    }
}

/// Resolve a one-off request from global defaults plus an optional rewrite profile.
pub(crate) fn resolve_one_off_llm_request_for_profile(
    base_config: &LlmConfig,
    profile: Option<&ProgramPromptProfile>,
    additional_layers: &[OneOffLlmProviderRequestLayer],
) -> OneOffLlmProviderRequest {
    let mut layers = Vec::with_capacity(additional_layers.len() + 1);
    layers.push(OneOffLlmProviderRequestLayer::from_profile(profile));
    layers.extend_from_slice(additional_layers);
    resolve_one_off_llm_request(base_config, &layers)
}

/// An action inherits the rewrite model only when its own route is absent.
/// Provider knobs remain profile-inheritable without changing payment source.
pub(crate) fn resolve_one_off_llm_request_for_action(
    base_config: &LlmConfig,
    profile: Option<&ProgramPromptProfile>,
    action: crate::model_selection::ModelAction,
    additional_layers: &[OneOffLlmProviderRequestLayer],
) -> OneOffLlmProviderRequest {
    let mut base = base_config.clone();
    base.selection = base.model_selections.get(&action).cloned().or_else(|| {
        (action != crate::model_selection::ModelAction::QuickAsk)
            .then_some(base.selection.clone())
            .flatten()
    });
    let mut layer = OneOffLlmProviderRequestLayer::from_profile(profile);
    layer.selection = profile
        .and_then(|profile| profile.model_selections.get(&action))
        .cloned()
        .or_else(|| {
            matches!(
                action,
                crate::model_selection::ModelAction::Rewrite
                    | crate::model_selection::ModelAction::HistoryAnalysis
                    | crate::model_selection::ModelAction::PromptTools
            )
            .then_some(layer.selection.clone())
            .flatten()
        });
    if let Some(profile) = profile {
        match action {
            crate::model_selection::ModelAction::QuickAsk => {
                layer.provider = profile.quick_ask_provider.clone();
                layer.model = profile.quick_ask_model.clone();
                layer.openai_reasoning_effort = profile.quick_ask_openai_reasoning_effort.clone();
                layer.gemini_thinking_budget = profile.quick_ask_gemini_thinking_budget;
                layer.gemini_thinking_level = profile.quick_ask_gemini_thinking_level.clone();
                layer.anthropic_thinking_budget = profile.quick_ask_anthropic_thinking_budget;
            }
            crate::model_selection::ModelAction::QuickReplace => {
                layer.provider = profile.quick_replace_provider.clone();
                layer.model = profile.quick_replace_model.clone();
            }
            crate::model_selection::ModelAction::IntentRouter => {
                layer.provider = profile
                    .router
                    .as_ref()
                    .and_then(|router| router.llm_provider.clone());
                layer.model = profile
                    .router
                    .as_ref()
                    .and_then(|router| router.llm_model.clone());
            }
            _ => {}
        }
    }
    let mut layers = vec![layer];
    layers.extend_from_slice(additional_layers);
    resolve_one_off_llm_request(&base, &layers)
}

/// Resolve a one-off request from the global rewrite config plus additional override layers.
///
/// Resolution order is:
/// 1. global rewrite config (`LlmConfig`)
/// 2. each provided layer in order
///
/// Later layers win. This lets commands express precedence like
/// global -> profile -> Prompt Lab override without each callsite restating every provider knob.
pub(crate) fn resolve_one_off_llm_request(
    base_config: &LlmConfig,
    layers: &[OneOffLlmProviderRequestLayer],
) -> OneOffLlmProviderRequest {
    let mut provider_id = normalize_optional_string(Some(base_config.provider.clone()))
        .unwrap_or_else(|| "openai".to_string());
    let mut model = normalize_optional_string(base_config.model.clone());
    let mut selection = base_config.selection.clone();
    let mut ollama_url = normalize_optional_string(base_config.ollama_url.clone());
    let mut openai_reasoning_effort =
        normalize_optional_string(base_config.openai_reasoning_effort.clone());
    let mut gemini_thinking_budget = base_config.gemini_thinking_budget;
    let mut gemini_thinking_level =
        normalize_optional_string(base_config.gemini_thinking_level.clone());
    let mut anthropic_thinking_budget = base_config.anthropic_thinking_budget;

    for layer in layers {
        selection = crate::model_selection::inherit_model_selection(
            selection.as_ref(),
            &[(
                layer.selection.as_ref(),
                layer.provider.as_deref(),
                layer.model.as_deref(),
            )],
        );
        if let Some(value) = normalize_optional_string(layer.provider.clone()) {
            provider_id = value;
        }
        if let Some(value) = normalize_optional_string(layer.model.clone()) {
            model = Some(value);
        }
        if let Some(value) = normalize_optional_string(layer.ollama_url.clone()) {
            ollama_url = Some(value);
        }
        if let Some(value) = normalize_optional_string(layer.openai_reasoning_effort.clone()) {
            openai_reasoning_effort = Some(value);
        }
        if let Some(value) = layer.gemini_thinking_budget {
            gemini_thinking_budget = Some(value);
        }
        if let Some(value) = normalize_optional_string(layer.gemini_thinking_level.clone()) {
            gemini_thinking_level = Some(value);
        }
        if let Some(value) = layer.anthropic_thinking_budget {
            anthropic_thinking_budget = Some(value);
        }
    }

    if let Some(choice) = selection.as_ref() {
        provider_id = choice.provider_id.clone();
        model = Some(choice.model_id.clone());
    }
    OneOffLlmProviderRequest {
        provider_id,
        params: LlmProviderParams {
            selection,
            model,
            // The no-timeout constructor ignores this field. We still keep the configured timeout
            // here so the same resolved request can also drive the normal one-off constructor.
            timeout: base_config.timeout,
            ollama_url,
            openai_reasoning_effort,
            gemini_thinking_budget,
            gemini_thinking_level,
            anthropic_thinking_budget,
        },
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::PromptSections;
    use std::time::Duration;

    #[test]
    fn quick_actions_and_router_use_their_own_profile_routes_and_knobs() {
        use crate::model_selection::{ModelAction, ModelSelection, ModelSource};
        let global = ModelSelection {
            source: ModelSource::Managed,
            provider_id: "groq".into(),
            model_id: "global".into(),
        };
        let mut base = LlmConfig {
            selection: Some(global.clone()),
            ollama_url: Some("http://local.test".into()),
            ..Default::default()
        };
        let mut p = profile("p");
        p.model_selections.insert(
            ModelAction::Rewrite,
            ModelSelection {
                source: ModelSource::Byok,
                provider_id: "anthropic".into(),
                model_id: "rewrite-only".into(),
            },
        );
        p.llm_provider = Some("anthropic".into());
        p.llm_model = Some("rewrite-only".into());
        p.quick_ask_provider = Some("openai".into());
        p.quick_ask_model = Some("ask".into());
        p.quick_ask_openai_reasoning_effort = Some("high".into());
        p.quick_ask_gemini_thinking_budget = Some(12);
        p.quick_ask_gemini_thinking_level = Some("low".into());
        p.quick_ask_anthropic_thinking_budget = Some(34);
        let ask =
            resolve_one_off_llm_request_for_action(&base, Some(&p), ModelAction::QuickAsk, &[]);
        assert_eq!(ask.provider_id, "openai");
        assert_eq!(ask.params.model.as_deref(), Some("ask"));
        assert!(ask.params.selection.is_none());
        assert_eq!(ask.params.openai_reasoning_effort.as_deref(), Some("high"));
        assert_eq!(ask.params.gemini_thinking_budget, Some(12));
        assert_eq!(ask.params.gemini_thinking_level.as_deref(), Some("low"));
        assert_eq!(ask.params.anthropic_thinking_budget, Some(34));
        base.model_selections
            .insert(ModelAction::QuickAsk, global.clone());
        assert_eq!(
            resolve_one_off_llm_request_for_action(&base, Some(&p), ModelAction::QuickAsk, &[])
                .params
                .selection
                .unwrap()
                .model_id,
            "ask"
        );
        p.quick_replace_provider = Some("ollama".into());
        p.quick_replace_model = Some("replace".into());
        let replace =
            resolve_one_off_llm_request_for_action(&base, Some(&p), ModelAction::QuickReplace, &[]);
        assert_eq!(replace.provider_id, "ollama");
        assert_eq!(replace.params.model.as_deref(), Some("replace"));
        assert_eq!(
            replace.params.selection.unwrap().source,
            ModelSource::Managed
        );
        assert_eq!(
            replace.params.ollama_url.as_deref(),
            Some("http://local.test")
        );
        let router =
            resolve_one_off_llm_request_for_action(&base, Some(&p), ModelAction::IntentRouter, &[]);
        assert_eq!(router.params.selection, Some(global.clone()));
        assert_eq!(router.provider_id, "groq");
        assert_eq!(router.params.model.as_deref(), Some("global"));
        p.router = Some(
            serde_json::from_value(
                serde_json::json!({"llm_provider":"openai","llm_model":"router"}),
            )
            .unwrap(),
        );
        let router =
            resolve_one_off_llm_request_for_action(&base, Some(&p), ModelAction::IntentRouter, &[]);
        assert_eq!(router.provider_id, "openai");
        assert_eq!(router.params.selection.unwrap().model_id, "router");
    }

    #[test]
    fn per_action_and_profile_routes_preserve_atomic_identity_over_stale_fields() {
        use crate::model_selection::{ModelAction, ModelSelection, ModelSource};
        let global = ModelSelection {
            source: ModelSource::Byok,
            provider_id: "openai".into(),
            model_id: "global".into(),
        };
        let action = ModelSelection {
            source: ModelSource::Managed,
            provider_id: "groq".into(),
            model_id: "action".into(),
        };
        let mut base = LlmConfig {
            selection: Some(global.clone()),
            ..Default::default()
        };
        base.model_selections
            .insert(ModelAction::HistoryAnalysis, action.clone());
        let resolved =
            resolve_one_off_llm_request_for_action(&base, None, ModelAction::HistoryAnalysis, &[]);
        assert_eq!(resolved.params.selection, Some(action.clone()));
        assert_eq!(resolved.provider_id, "groq");
        assert_eq!(resolved.params.model.as_deref(), Some("action"));
        let mut p = profile("p");
        p.model_selections
            .insert(ModelAction::Rewrite, global.clone());
        assert_eq!(
            resolve_one_off_llm_request_for_action(&base, Some(&p), ModelAction::PromptTools, &[])
                .params
                .selection,
            Some(global)
        );
        p.model_selections
            .insert(ModelAction::PromptTools, action.clone());
        p.llm_provider = Some("stale".into());
        p.llm_model = Some("stale".into());
        let resolved =
            resolve_one_off_llm_request_for_action(&base, Some(&p), ModelAction::PromptTools, &[]);
        assert_eq!(resolved.params.selection, Some(action.clone()));
        assert_eq!(resolved.provider_id, "groq");
        let override_layer = OneOffLlmProviderRequestLayer {
            model: Some("override".into()),
            ..Default::default()
        };
        assert_eq!(
            resolve_one_off_llm_request_for_action(
                &base,
                Some(&p),
                ModelAction::PromptTools,
                &[override_layer]
            )
            .params
            .selection
            .unwrap()
            .model_id,
            "override"
        );
    }

    fn profile(id: &str) -> ProgramPromptProfile {
        ProgramPromptProfile {
            model_selections: Default::default(),
            id: id.to_string(),
            name: id.to_string(),
            program_paths: Vec::new(),
            prompts: PromptSections::default(),
            presets: Vec::new(),
            default_preset_id: None,
            default_preset_description: None,
            default_target_rewrite_llm_enabled: true,
            active_preset_id: None,
            router: None,
            rewrite_llm_enabled: None,
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

    #[test]
    fn profile_request_prefers_profile_values_over_global_defaults() {
        let mut base = LlmConfig::default();
        base.provider = "openai".to_string();
        base.model = Some("gpt-5".to_string());
        base.timeout = Duration::from_secs(45);
        base.openai_reasoning_effort = Some("low".to_string());
        base.gemini_thinking_budget = Some(11);
        base.gemini_thinking_level = Some("auto".to_string());
        base.anthropic_thinking_budget = Some(22);

        let mut rewrite_profile = profile("rewrite");
        rewrite_profile.llm_provider = Some("groq".to_string());
        rewrite_profile.llm_model = Some("llama-3.3-70b".to_string());
        rewrite_profile.openai_reasoning_effort = Some("high".to_string());
        rewrite_profile.gemini_thinking_budget = Some(33);
        rewrite_profile.gemini_thinking_level = Some("medium".to_string());
        rewrite_profile.anthropic_thinking_budget = Some(44);

        let request = resolve_one_off_llm_request_for_profile(&base, Some(&rewrite_profile), &[]);

        assert_eq!(request.provider_id, "groq");
        assert_eq!(request.params.model.as_deref(), Some("llama-3.3-70b"));
        assert_eq!(request.params.timeout, Duration::from_secs(45));
        assert_eq!(
            request.params.openai_reasoning_effort.as_deref(),
            Some("high")
        );
        assert_eq!(request.params.gemini_thinking_budget, Some(33));
        assert_eq!(
            request.params.gemini_thinking_level.as_deref(),
            Some("medium")
        );
        assert_eq!(request.params.anthropic_thinking_budget, Some(44));
    }

    #[test]
    fn later_layers_override_profile_and_blank_strings_do_not_clobber() {
        let mut base = LlmConfig::default();
        base.provider = "openai".to_string();
        base.model = Some("gpt-5".to_string());
        base.timeout = Duration::from_secs(30);
        base.openai_reasoning_effort = Some("low".to_string());
        base.gemini_thinking_budget = Some(10);
        base.gemini_thinking_level = Some("auto".to_string());
        base.anthropic_thinking_budget = Some(20);

        let mut rewrite_profile = profile("rewrite");
        rewrite_profile.llm_provider = Some("groq".to_string());
        rewrite_profile.llm_model = Some("llama".to_string());
        rewrite_profile.openai_reasoning_effort = Some("medium".to_string());

        let request = resolve_one_off_llm_request_for_profile(
            &base,
            Some(&rewrite_profile),
            &[
                OneOffLlmProviderRequestLayer {
                    provider: Some("   ".to_string()),
                    model: Some("   ".to_string()),
                    openai_reasoning_effort: Some("   ".to_string()),
                    gemini_thinking_budget: Some(123),
                    gemini_thinking_level: Some("   ".to_string()),
                    anthropic_thinking_budget: Some(456),
                    ..Default::default()
                },
                OneOffLlmProviderRequestLayer {
                    provider: Some("anthropic".to_string()),
                    model: Some("claude-sonnet-4-5".to_string()),
                    gemini_thinking_level: Some("high".to_string()),
                    ..Default::default()
                },
            ],
        );

        assert_eq!(request.provider_id, "anthropic");
        assert_eq!(request.params.model.as_deref(), Some("claude-sonnet-4-5"));
        // Blank-string overrides are ignored, so the profile value survives here.
        assert_eq!(
            request.params.openai_reasoning_effort.as_deref(),
            Some("medium")
        );
        assert_eq!(request.params.gemini_thinking_budget, Some(123));
        assert_eq!(
            request.params.gemini_thinking_level.as_deref(),
            Some("high")
        );
        assert_eq!(request.params.anthropic_thinking_budget, Some(456));
    }

    #[test]
    fn direct_request_resolution_uses_global_defaults_when_no_layers_override_them() {
        let mut base = LlmConfig::default();
        base.provider = "gemini".to_string();
        base.model = Some("gemini-2.5-flash".to_string());
        base.ollama_url = Some(" http://localhost:11434 ".to_string());
        base.gemini_thinking_budget = Some(77);
        base.gemini_thinking_level = Some("auto".to_string());

        let request = resolve_one_off_llm_request(&base, &[]);

        assert_eq!(request.provider_id, "gemini");
        assert_eq!(request.params.model.as_deref(), Some("gemini-2.5-flash"));
        assert_eq!(
            request.params.ollama_url.as_deref(),
            Some("http://localhost:11434")
        );
        assert_eq!(request.params.gemini_thinking_budget, Some(77));
        assert_eq!(
            request.params.gemini_thinking_level.as_deref(),
            Some("auto")
        );
    }
}
