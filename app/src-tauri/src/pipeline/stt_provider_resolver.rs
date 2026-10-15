//! STT provider resolution for transcription flows.
//!
//! This module owns the decisions needed to turn profile/preset/global STT
//! settings plus optional per-run overrides into a ready-to-use provider. Keeping
//! this in one place gives callers a small interface for a surprisingly fussy
//! implementation: cache keys, managed inference routing, Local Whisper special
//! cases, request-log provider/model fields, and global fallback behavior.

use std::sync::Arc;
use std::time::Duration;

use crate::llm::{ProgramPreset, ProgramPromptProfile};
use crate::stt::SttProvider;

use super::{
    canonicalize_stt_provider_id, local_provider_lifecycle as local_provider,
    managed_gateway_ready, resolve_stt_provider_for_runtime, stt_provider, PipelineError,
    PipelineInner,
};

/// Request to resolve the STT provider for a transcription attempt.
pub(super) struct SttProviderResolutionRequest<'a> {
    /// Active profile for this transcription, if one was selected.
    pub active_profile: Option<&'a ProgramPromptProfile>,
    /// Active preset for this transcription, if one was selected.
    pub active_preset: Option<&'a ProgramPreset>,
    /// Per-run provider override, used by CLI/diagnostic paths.
    pub forced_provider: Option<&'a str>,
    /// Per-run model override, used by CLI/diagnostic paths.
    pub forced_model: Option<&'a str>,
    /// Snapshot language, including explicit auto; None retains profile inheritance.
    pub forced_language: Option<&'a str>,
}

/// Fully-resolved STT provider details for a transcription attempt.
#[derive(Clone)]
pub(super) struct ResolvedSttProvider {
    pub provider: Arc<dyn SttProvider>,
    pub provider_id: String,
    pub model: Option<String>,
    pub language: Option<String>,
    pub timeout: Duration,
    pub managed: bool,
    pub managed_token: Option<String>,
}

/// Resolve effective STT settings, apply per-run overrides, update request-log
/// provider/model fields and create the exact selected provider. An unavailable
/// profile/override never silently falls back to the dictation model.
pub(super) fn resolve_stt_provider_for_transcription(
    inner: &mut PipelineInner,
    request: SttProviderResolutionRequest<'_>,
) -> Result<ResolvedSttProvider, PipelineError> {
    if let Some(error) = &inner.config.model_selection_error {
        return Err(PipelineError::Config(error.clone()));
    }
    let mut effective =
        inner.resolve_effective_stt_settings(request.active_profile, request.active_preset);

    apply_forced_overrides(
        &mut effective.provider_id,
        &mut effective.model,
        request.forced_provider,
        request.forced_model,
    );
    if request.forced_provider.is_some() || request.forced_model.is_some() {
        effective.selection = None;
    }
    if let Some(choice) = &effective.selection {
        choice.validate().map_err(PipelineError::Config)?;
    }
    if let Some(language) = request.forced_language {
        effective.language = super::normalize_stt_language_setting(Some(language.to_string()));
    }

    let provider_id_used = if request.forced_provider.is_some() || effective.selection.is_some() {
        effective.provider_id.clone()
    } else {
        resolve_stt_provider_for_runtime(
            &inner.config,
            &effective.provider_id,
            effective.model.as_deref(),
        )
    };
    let model_used = model_for_log(inner, provider_id_used.as_str(), effective.model.clone());
    if provider_id_used == "ivritai" {
        // Model-owned language; keep the stored profile/retry preference intact
        // while reporting and caching the language actually sent to the service.
        effective.language = Some("he".into());
    }
    let language_used = effective.language.clone();

    log_stt_provider_selection(
        inner,
        provider_id_used.as_str(),
        model_used.clone(),
        language_used.as_deref(),
    );

    // A meeting's explicitly selected local model must not follow later changes
    // to the dictation model. Use the existing model directory, never a UI path.
    #[cfg(feature = "local-whisper")]
    if (request.forced_provider.is_some() || effective.selection.is_some())
        && provider_id_used == local_provider::LOCAL_WHISPER_PROVIDER_ID
    {
        let normalize = |value: &str| {
            value
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>()
        };
        let selected = crate::stt::WhisperModel::all()
            .into_iter()
            .find(|model| {
                normalize(&format!("{model:?}"))
                    == normalize(effective.model.as_deref().unwrap_or(""))
            })
            .ok_or_else(|| PipelineError::Config("Unknown meeting Whisper model".into()))?;
        let directory = inner
            .config
            .whisper_model_path
            .as_ref()
            .and_then(|p| p.parent())
            .ok_or_else(|| {
                PipelineError::Config("Configure a downloaded Whisper model first".into())
            })?;
        let path = directory.join(selected.filename());
        let key = local_provider::local_whisper_cache_key_for_language(
            &path.to_string_lossy(),
            effective.language.as_deref(),
        );
        let provider = if let Some(provider) = inner.stt_provider_cache.get(&key) {
            provider.clone()
        } else {
            if let Some(message) = local_provider::manual_unloaded_error(
                "local-whisper",
                &inner.config.local_whisper_load_mode,
                false,
            ) {
                return Err(PipelineError::Config(message.into()));
            }
            let provider: Arc<dyn SttProvider> = Arc::new(
                crate::stt::LocalWhisperProvider::with_config(crate::stt::LocalWhisperConfig {
                    model_path: path,
                    language: effective.language.clone(),
                    transcription_prompt: inner.config.stt_transcription_prompt.clone(),
                    ..Default::default()
                })
                .map_err(PipelineError::Stt)?,
            );
            inner.stt_provider_cache.insert(key, provider.clone());
            provider
        };
        return Ok(ResolvedSttProvider {
            provider,
            provider_id: provider_id_used,
            model: effective.model,
            language: effective.language,
            timeout: effective.timeout,
            managed: false,
            managed_token: None,
        });
    }

    let (provider, managed) = get_or_create_resolved_stt_provider(
        inner,
        &provider_id_used,
        effective.model.clone(),
        effective.language.clone(),
        effective.selection.as_ref().map(|choice| choice.source),
    )
    .map_err(|error| {
        inner.set_error(
            "The selected transcription provider is unavailable; the model choice has not changed",
        );
        error
    })?;

    if effective.input_mode == crate::settings::SttInputMode::AfterRecording
        && provider.requires_streaming()
    {
        inner.set_error("This model requires realtime input; choose Auto or a batch-capable model");
        return Err(PipelineError::Config(
            "After recording is unavailable for this realtime-only model".into(),
        ));
    }
    Ok(ResolvedSttProvider {
        provider,
        provider_id: provider_id_used,
        model: model_used,
        language: language_used,
        timeout: effective.timeout,
        managed,
        managed_token: if managed {
            inner.config.managed_inference_access_token.clone()
        } else {
            None
        },
    })
}

fn apply_forced_overrides(
    provider_id: &mut String,
    model: &mut Option<String>,
    forced_provider: Option<&str>,
    forced_model: Option<&str>,
) {
    if let Some(provider) = forced_provider.map(str::trim).filter(|p| !p.is_empty()) {
        *provider_id = canonicalize_stt_provider_id(&provider.to_lowercase());
    }

    if let Some(forced_model) = forced_model {
        let forced_model = forced_model.trim();
        *model = if forced_model.is_empty() {
            None
        } else {
            Some(forced_model.to_string())
        };
    }
}

fn model_for_log(
    inner: &PipelineInner,
    provider_id: &str,
    model: Option<String>,
) -> Option<String> {
    #[cfg(feature = "local-whisper")]
    if provider_id == "local-whisper" {
        return inner
            .config
            .whisper_model_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|f| f.to_string_lossy().to_string());
    }

    let _ = inner;
    let _ = provider_id;
    model
}

fn log_stt_provider_selection(
    inner: &PipelineInner,
    provider_id: &str,
    model: Option<String>,
    language: Option<&str>,
) {
    if let Some(store) = inner.config.request_log_store.as_ref() {
        store.with_current(|log| {
            log.stt_provider = provider_id.to_string();
            log.stt_model = model;
            log.stt_language_hint = Some(language.unwrap_or("auto").to_owned());
        });
    }
}

pub(super) fn managed_stt_model_supported(provider_id: &str, model: Option<&str>) -> bool {
    if provider_id == "ivritai" {
        return model == Some(crate::stt::ivritai::MODEL);
    }
    if provider_id == "yiddishlabs" {
        return matches!(
            model,
            Some(crate::stt::yiddishlabs::FLASH_MODEL | crate::stt::yiddishlabs::STANDARD_MODEL)
        );
    }
    // Transport support is not entitlement: Edge still validates its live catalog.
    if provider_id == "openai" && model == Some("gpt-4o-transcribe-diarize") {
        return true;
    }
    if provider_id != "groq" {
        return false;
    }

    matches!(
        model.unwrap_or("whisper-large-v3-turbo"),
        "whisper-large-v3-turbo" | "whisper-large-v3"
    )
}

pub(super) fn stt_provider_cache_key(
    inner: &PipelineInner,
    provider_id: &str,
    model: Option<String>,
    language: Option<String>,
) -> String {
    // NOTE: for Local Whisper, the "model" setting is not meaningful (Whisper model is
    // selected via `whisper_model_path`). It also does not use cloud live-output
    // provider construction, so keep this key aligned with manual preload/status checks.
    if provider_id == local_provider::LOCAL_WHISPER_PROVIDER_ID {
        return inner.local_whisper_cache_key_for_language(language.as_deref());
    }

    let language_key = language.unwrap_or_else(|| "<auto>".to_string());
    let model_key = model.unwrap_or_else(|| "<default>".to_string());

    format!(
        "{}::{}::{}::live={}::managed={}",
        provider_id,
        model_key,
        language_key,
        inner.config.stt_live_output,
        inner.config.managed_inference_enabled && inner.config.managed_stt_preferred
    )
}

pub(super) fn get_or_create_stt_provider(
    inner: &mut PipelineInner,
    provider_id: &str,
    model: Option<String>,
    language: Option<String>,
) -> Result<Arc<dyn SttProvider>, PipelineError> {
    let provider_id =
        resolve_stt_provider_for_runtime(&inner.config, provider_id, model.as_deref());
    get_or_create_resolved_stt_provider(inner, &provider_id, model, language, None)
        .map(|(provider, _)| provider)
}

#[cfg(test)]
pub(super) fn get_or_create_selected_stt_provider(
    inner: &mut PipelineInner,
    choice: &crate::model_selection::ModelSelection,
    language: Option<String>,
) -> Result<Arc<dyn SttProvider>, PipelineError> {
    choice.validate().map_err(PipelineError::Config)?;
    get_or_create_resolved_stt_provider(
        inner,
        &choice.provider_id,
        Some(choice.model_id.clone()),
        language,
        Some(choice.source),
    )
    .map(|(provider, _)| provider)
}

pub(super) fn managed_transport_active(
    inner: &PipelineInner,
    provider_id: &str,
    model: Option<&str>,
    source: Option<crate::model_selection::ModelSource>,
) -> Result<bool, PipelineError> {
    let ready = inner.config.managed_inference_enabled && managed_gateway_ready(&inner.config);
    let supported = managed_stt_model_supported(provider_id, model)
        && !local_provider::bypasses_managed_transport(provider_id);
    match source {
        Some(crate::model_selection::ModelSource::Byok) => Ok(false),
        Some(crate::model_selection::ModelSource::Managed) if !ready || !supported => Err(
            PipelineError::Config("The selected Managed transcription model is unavailable. Sign in or refresh access; your model choice has not changed".into()),
        ),
        Some(crate::model_selection::ModelSource::Managed) => Ok(true),
        None => Ok(ready && inner.config.managed_stt_preferred && supported),
    }
}

pub(super) fn get_or_create_resolved_stt_provider(
    inner: &mut PipelineInner,
    provider_id: &str,
    model: Option<String>,
    language: Option<String>,
    source: Option<crate::model_selection::ModelSource>,
) -> Result<(Arc<dyn SttProvider>, bool), PipelineError> {
    if let Some(error) = &inner.config.model_selection_error {
        return Err(PipelineError::Config(error.clone()));
    }
    let provider_id = canonicalize_stt_provider_id(provider_id);
    let managed_transport_active =
        managed_transport_active(inner, &provider_id, model.as_deref(), source)?;

    if managed_transport_active {
        if let Some(store) = &inner.config.request_log_store {
            let _ = store.with_current(|log| {
                log.managed_inference = true;
            });
        }
    }

    let cache_key = match source {
        Some(source) => serde_json::json!([
            provider_id,
            model,
            language,
            inner.config.stt_live_output,
            source
        ])
        .to_string(),
        None => {
            stt_provider_cache_key(inner, provider_id.as_str(), model.clone(), language.clone())
        }
    };
    if let Some(provider) = inner.stt_provider_cache.get(&cache_key) {
        return Ok((provider.clone(), managed_transport_active));
    }

    // Manual local-whisper mode: require explicit preload to avoid surprise UI stalls
    // during stop/transcribe.
    if let Some(message) = local_provider::manual_unloaded_error(
        provider_id.as_str(),
        &inner.config.local_whisper_load_mode,
        false,
    ) {
        return Err(PipelineError::Config(message.to_string()));
    }

    #[cfg(feature = "local-whisper")]
    if provider_id == local_provider::LOCAL_WHISPER_PROVIDER_ID {
        if let Some(message) = local_provider::local_whisper_model_unavailable_error(
            provider_id.as_str(),
            true,
            inner.config.whisper_model_path.is_some(),
        ) {
            return Err(PipelineError::Config(message.to_string()));
        }

        if let Some(model_path) = &inner.config.whisper_model_path {
            let provider =
                crate::stt::LocalWhisperProvider::with_config(crate::stt::LocalWhisperConfig {
                    model_path: model_path.clone(),
                    language: language.clone(),
                    transcription_prompt: inner.config.stt_transcription_prompt.clone(),
                    ..Default::default()
                })
                .map_err(|e| PipelineError::Config(format!("Local Whisper init failed: {}", e)))?;
            let provider = Arc::new(provider);
            inner.stt_provider_cache.insert(cache_key, provider.clone());
            return Ok((provider, managed_transport_active));
        }

        return Err(PipelineError::Config(
            "Local Whisper selected but no model path configured".to_string(),
        ));
    }

    if provider_id == local_provider::WHISPER_SERVER_PROVIDER_ID {
        let base_url = inner
            .config
            .whisper_server_base_url
            .clone()
            .unwrap_or_default();

        let provider = crate::stt::WhisperServerSttProvider::with_client(
            stt_provider::build_stt_client(&inner.config.proxy_settings)?,
            base_url,
            model,
            language,
            inner.config.stt_transcription_prompt.clone(),
        )
        .map_err(|e| PipelineError::Config(format!("Whisper server init failed: {}", e)))?
        .with_request_log_store(inner.config.request_log_store.clone());

        let provider = Arc::new(provider);
        inner.stt_provider_cache.insert(cache_key, provider.clone());
        return Ok((provider, managed_transport_active));
    }

    if provider_id.starts_with("custom_") {
        let definition = inner
            .config
            .llm_config
            .custom_providers
            .iter()
            .find(|p| p.id == provider_id)
            .ok_or_else(|| {
                PipelineError::Config("Custom provider is no longer configured".into())
            })?;
        let model = model
            .filter(|m| definition.stt_models.contains(m))
            .ok_or_else(|| {
                PipelineError::Config("Select a configured custom transcription model".into())
            })?;
        let key = inner
            .config
            .stt_api_keys
            .get(&provider_id)
            .filter(|k| !k.is_empty())
            .ok_or_else(|| {
                PipelineError::Config("Add this custom provider's API key in Settings".into())
            })?;
        let provider = crate::stt::WhisperServerSttProvider::with_client(
            crate::network::build_custom_provider_client(&inner.config.proxy_settings)
                .map_err(PipelineError::Config)?,
            definition.base_url.clone(),
            Some(model),
            language,
            inner.config.stt_transcription_prompt.clone(),
        )
        .map_err(PipelineError::Stt)?
        .with_api_key(key.clone())
        .with_request_log_store(inner.config.request_log_store.clone());
        let provider: Arc<dyn SttProvider> = Arc::new(provider);
        inner.stt_provider_cache.insert(cache_key, provider.clone());
        return Ok((provider, managed_transport_active));
    }

    // Cloud providers use the common factory.
    let api_key = if managed_transport_active {
        inner
            .config
            .managed_inference_access_token
            .clone()
            .unwrap_or_default()
    } else {
        inner
            .config
            .stt_api_keys
            .get(if provider_id == "yiddishlabs" {
                if model.as_deref() == Some(crate::stt::yiddishlabs::STANDARD_MODEL) {
                    crate::stt::yiddishlabs::STANDARD_KEY_SLOT
                } else {
                    crate::stt::yiddishlabs::FLASH_KEY_SLOT
                }
            } else {
                &provider_id
            })
            .cloned()
            .unwrap_or_default()
    };

    let client = stt_provider::build_stt_client(&inner.config.proxy_settings)?;

    let provider = stt_provider::create_cloud_stt_provider(
        client,
        stt_provider::SttProviderParams {
            provider_id,
            model,
            language,
            api_key,
            proxy_settings: inner.config.proxy_settings.clone(),
            managed_gateway_url: if managed_transport_active {
                inner.config.managed_inference_gateway_url.clone()
            } else {
                None
            },
            transcription_prompt: inner.config.stt_transcription_prompt.clone(),
            request_log_store: inner.config.request_log_store.clone(),
        },
    )?;

    inner.stt_provider_cache.insert(cache_key, provider.clone());
    Ok((provider, managed_transport_active))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{ProgramPreset, ProgramPromptProfile, PromptSections};
    use crate::pipeline::config::PipelineConfig;
    use crate::request_log::RequestLogStore;
    use crate::settings::{IntentRouterSettings, IntentRouterStrategy};
    use crate::stt::{AudioFormat, SttError};
    use async_trait::async_trait;

    #[test]
    fn yiddishlabs_requires_the_key_slot_for_the_exact_selected_model() {
        let mut inner = PipelineInner::new(PipelineConfig::default());
        inner.config.stt_api_keys.insert(
            crate::stt::yiddishlabs::FLASH_KEY_SLOT.into(),
            "synthetic-flash-key".into(),
        );
        let source = Some(crate::model_selection::ModelSource::Byok);
        assert!(get_or_create_resolved_stt_provider(
            &mut inner,
            "yiddishlabs",
            Some(crate::stt::yiddishlabs::STANDARD_MODEL.into()),
            None,
            source
        )
        .is_err());
        let (flash, managed) = get_or_create_resolved_stt_provider(
            &mut inner,
            "yiddishlabs",
            Some(crate::stt::yiddishlabs::FLASH_MODEL.into()),
            None,
            source,
        )
        .unwrap();
        assert_eq!(flash.name(), "yiddishlabs");
        assert!(!managed);
        assert!(!flash.supports_streaming());
        inner.config.stt_api_keys.insert(
            crate::stt::yiddishlabs::STANDARD_KEY_SLOT.into(),
            "synthetic-standard-key".into(),
        );
        let (standard, managed) = get_or_create_resolved_stt_provider(
            &mut inner,
            "yiddishlabs",
            Some(crate::stt::yiddishlabs::STANDARD_MODEL.into()),
            None,
            source,
        )
        .unwrap();
        assert_eq!(standard.name(), "yiddishlabs");
        assert!(!managed);
        assert!(!standard.supports_streaming());
    }

    #[test]
    fn custom_transcription_requires_a_saved_raw_id_and_its_own_key() {
        let mut inner = PipelineInner::new(PipelineConfig::default());
        let source = Some(crate::model_selection::ModelSource::Byok);
        assert!(get_or_create_resolved_stt_provider(
            &mut inner,
            "custom_fixture",
            Some("speech".into()),
            None,
            source
        )
        .is_err());
        inner.config.llm_config.custom_providers.push(serde_json::from_value(serde_json::json!({
            "id":"custom_fixture", "name":"Synthetic provider", "base_url":"http://127.0.0.1:1", "stt_models":["speech"], "llm_models":[], "stt_model_names":{"speech":"Friendly name"}
        })).unwrap());
        for model in [None, Some("Friendly name".into()), Some("speech".into())] {
            assert!(get_or_create_resolved_stt_provider(
                &mut inner,
                "custom_fixture",
                model,
                None,
                source
            )
            .is_err());
        }
        inner
            .config
            .stt_api_keys
            .insert("custom_fixture".into(), "synthetic-key".into());
        let (provider, managed) = get_or_create_resolved_stt_provider(
            &mut inner,
            "custom_fixture",
            Some("speech".into()),
            None,
            source,
        )
        .unwrap();
        assert_eq!(provider.name(), "custom");
        assert!(!managed);
    }

    struct FakeSttProvider;

    #[async_trait]
    impl SttProvider for FakeSttProvider {
        async fn transcribe(
            &self,
            _audio: &[u8],
            _format: &AudioFormat,
        ) -> Result<String, SttError> {
            Ok("fake transcript".to_string())
        }

        fn name(&self) -> &'static str {
            "fake"
        }
    }

    fn insert_cached_fake_provider(
        inner: &mut PipelineInner,
        provider_id: &str,
        model: Option<String>,
        language: Option<String>,
    ) {
        let cache_key = stt_provider_cache_key(inner, provider_id, model, language);
        inner
            .stt_provider_cache
            .insert(cache_key, Arc::new(FakeSttProvider));
    }

    fn profile_with_stt(
        provider: Option<&str>,
        model: Option<&str>,
        language: Option<&str>,
        timeout_seconds: Option<f64>,
    ) -> ProgramPromptProfile {
        ProgramPromptProfile {
            model_selections: Default::default(),
            id: "profile".to_string(),
            name: "Profile".to_string(),
            program_paths: vec![],
            rewrite_llm_enabled: Some(true),
            rewrite_include_clipboard_context: None,
            stt_provider: provider.map(str::to_string),
            stt_model: model.map(str::to_string),
            stt_language: language.map(str::to_string),
            stt_input_mode: None,
            stt_timeout_seconds: timeout_seconds,
            llm_provider: None,
            llm_model: None,
            openai_reasoning_effort: None,
            gemini_thinking_budget: None,
            gemini_thinking_level: None,
            anthropic_thinking_budget: None,
            prompts: PromptSections::default(),
            presets: vec![],
            default_preset_id: None,
            default_preset_description: None,
            default_target_rewrite_llm_enabled: true,
            active_preset_id: None,
            router: Some(IntentRouterSettings {
                enabled: false,
                strategy: IntentRouterStrategy::Embeddings,
                embedding_provider: None,
                embedding_model: None,
                pick_highest_score: false,
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

    fn preset_with_stt(
        provider: Option<&str>,
        model: Option<&str>,
        language: Option<&str>,
        timeout_seconds: Option<f64>,
    ) -> ProgramPreset {
        ProgramPreset {
            model_selections: Default::default(),
            id: "preset".to_string(),
            name: "Preset".to_string(),
            routing_hints: vec![],
            prompts: PromptSections::default(),
            rewrite_llm_enabled: true,
            stt_provider: provider.map(str::to_string),
            stt_model: model.map(str::to_string),
            stt_language: language.map(str::to_string),
            stt_input_mode: None,
            stt_timeout_seconds: timeout_seconds,
            llm_provider: None,
            llm_model: None,
            openai_reasoning_effort: None,
            gemini_thinking_budget: None,
            gemini_thinking_level: None,
            anthropic_thinking_budget: None,
        }
    }

    #[test]
    fn forced_provider_is_trimmed_and_canonicalized() {
        let mut provider_id = "openai".to_string();
        let mut model = Some("base".to_string());

        apply_forced_overrides(&mut provider_id, &mut model, Some(" Groq "), None);

        assert_eq!(provider_id, "groq");
        assert_eq!(model.as_deref(), Some("base"));
    }

    #[test]
    fn invalid_local_endpoint_fails_without_substituting_or_caching_a_provider() {
        let mut inner = PipelineInner::new(PipelineConfig::default());
        inner.config.whisper_server_base_url = Some("not-an-endpoint".into());
        let error = get_or_create_resolved_stt_provider(
            &mut inner,
            local_provider::WHISPER_SERVER_PROVIDER_ID,
            Some("whisper-1".into()),
            None,
            Some(crate::model_selection::ModelSource::Byok),
        )
        .err()
        .unwrap();
        assert!(matches!(error, PipelineError::Config(_)));
        assert!(error.to_string().contains("Whisper server init failed"));
        assert!(inner.stt_provider_cache.is_empty());
        inner.config.whisper_server_base_url = Some("http://127.0.0.1:9/v1".into());
        let (provider, managed) = get_or_create_resolved_stt_provider(
            &mut inner,
            local_provider::WHISPER_SERVER_PROVIDER_ID,
            Some("whisper-1".into()),
            None,
            Some(crate::model_selection::ModelSource::Byok),
        )
        .unwrap();
        assert_eq!(provider.name(), "whisper-server");
        assert!(!managed);
        assert_eq!(inner.stt_provider_cache.len(), 1);
    }

    #[test]
    fn blank_forced_model_clears_model_override() {
        let mut provider_id = "openai".to_string();
        let mut model = Some("whisper-1".to_string());

        apply_forced_overrides(&mut provider_id, &mut model, None, Some("   "));

        assert_eq!(provider_id, "openai");
        assert_eq!(model, None);
    }

    #[test]
    fn local_whisper_cache_key_matches_manual_load_key_and_ignores_live_output() {
        let mut inner = PipelineInner::new(PipelineConfig {
            stt_provider: "local-whisper".to_string(),
            stt_language: Some("en".to_string()),
            stt_live_output: false,
            ..Default::default()
        });

        let manual_key = inner.local_whisper_cache_key();
        let resolver_key = stt_provider_cache_key(
            &inner,
            "local-whisper",
            Some("ignored-cloud-model".to_string()),
            Some("en".to_string()),
        );
        assert_eq!(resolver_key, manual_key);

        inner.config.stt_live_output = true;
        let live_output_key = stt_provider_cache_key(
            &inner,
            "local-whisper",
            Some("ignored-cloud-model".to_string()),
            Some("en".to_string()),
        );
        assert_eq!(live_output_key, manual_key);
    }

    #[test]
    fn managed_stt_support_is_limited_to_cataloged_groq_models() {
        assert!(managed_stt_model_supported(
            "ivritai",
            Some(crate::stt::ivritai::MODEL)
        ));
        assert!(!managed_stt_model_supported("ivritai", None));
        assert!(!managed_stt_model_supported("ivritai", Some("wrong")));
        assert!(managed_stt_model_supported(
            "groq",
            Some("whisper-large-v3-turbo")
        ));
        assert!(managed_stt_model_supported(
            "groq",
            Some("whisper-large-v3")
        ));
        assert!(!managed_stt_model_supported("openai", Some("whisper-1")));
        assert!(!managed_stt_model_supported(
            "groq",
            Some("distil-whisper-large-v3-en")
        ));
    }

    #[test]
    fn managed_stt_preference_is_part_of_the_provider_cache_key() {
        let mut inner = PipelineInner::new(PipelineConfig {
            managed_inference_enabled: true,
            managed_stt_preferred: true,
            ..Default::default()
        });
        let managed_key = stt_provider_cache_key(
            &inner,
            "groq",
            Some("whisper-large-v3-turbo".to_string()),
            Some("en".to_string()),
        );

        inner.config.managed_stt_preferred = false;
        let byok_key = stt_provider_cache_key(
            &inner,
            "groq",
            Some("whisper-large-v3-turbo".to_string()),
            Some("en".to_string()),
        );

        assert_ne!(managed_key, byok_key);
        assert!(managed_key.ends_with("managed=true"));
        assert!(byok_key.ends_with("managed=false"));
    }

    #[test]
    fn preset_overrides_profile_and_global_settings() {
        let mut inner = PipelineInner::new(PipelineConfig {
            stt_provider: "openai".to_string(),
            stt_model: Some("global-model".to_string()),
            stt_language: Some("en".to_string()),
            transcription_timeout: Duration::from_secs(30),
            ..Default::default()
        });
        insert_cached_fake_provider(
            &mut inner,
            "groq",
            Some("preset-model".to_string()),
            Some("es".to_string()),
        );

        let profile = profile_with_stt(
            Some("deepgram"),
            Some("profile-model"),
            Some("fr"),
            Some(11.0),
        );
        let preset = preset_with_stt(Some("groq"), Some("preset-model"), Some("es"), Some(7.0));

        let resolved = resolve_stt_provider_for_transcription(
            &mut inner,
            SttProviderResolutionRequest {
                active_profile: Some(&profile),
                active_preset: Some(&preset),
                forced_provider: None,
                forced_model: None,
                forced_language: None,
            },
        )
        .expect("preset provider should resolve from cache");

        assert_eq!(resolved.provider_id, "groq");
        assert_eq!(resolved.model.as_deref(), Some("preset-model"));
        assert_eq!(resolved.language.as_deref(), Some("es"));
        assert_eq!(resolved.timeout, Duration::from_secs(7));
    }

    #[test]
    fn recording_language_snapshot_overrides_inherited_hints_and_cache_identity() {
        let mut inner = PipelineInner::new(PipelineConfig {
            stt_provider: "groq".into(),
            stt_language: Some("en".into()),
            ..Default::default()
        });
        let profile = profile_with_stt(None, None, Some("fr"), None);
        let preset = preset_with_stt(None, None, Some("es"), None);
        for (hint, expected) in [("auto", None), ("yi", Some("yi")), ("he", Some("he"))] {
            insert_cached_fake_provider(
                &mut inner,
                "groq",
                Some("snapshot-model".into()),
                expected.map(String::from),
            );
            let resolved = resolve_stt_provider_for_transcription(
                &mut inner,
                SttProviderResolutionRequest {
                    active_profile: Some(&profile),
                    active_preset: Some(&preset),
                    forced_provider: Some("groq"),
                    forced_model: Some("snapshot-model"),
                    forced_language: Some(hint),
                },
            )
            .unwrap();
            assert_eq!(resolved.language.as_deref(), expected);
            assert_eq!(resolved.model.as_deref(), Some("snapshot-model"));
            assert_eq!(inner.config.stt_language.as_deref(), Some("en"));
        }
    }

    #[test]
    fn ivritai_resolution_uses_hebrew_without_mutating_inherited_or_snapshot_hints() {
        let store = RequestLogStore::new();
        store.start_request("synthetic".into(), None);
        let mut inner = PipelineInner::new(PipelineConfig {
            stt_provider: "ivritai".into(),
            stt_model: Some(crate::stt::ivritai::MODEL.into()),
            stt_language: Some("en".into()),
            request_log_store: Some(store.clone()),
            ..Default::default()
        });
        insert_cached_fake_provider(
            &mut inner,
            "ivritai",
            Some(crate::stt::ivritai::MODEL.into()),
            Some("he".into()),
        );
        let profile = profile_with_stt(None, None, Some("yi"), None);
        for forced_language in [None, Some("en"), Some("auto"), Some("he")] {
            let resolved = resolve_stt_provider_for_transcription(
                &mut inner,
                SttProviderResolutionRequest {
                    active_profile: Some(&profile),
                    active_preset: None,
                    forced_provider: Some("ivritai"),
                    forced_model: Some(crate::stt::ivritai::MODEL),
                    forced_language,
                },
            )
            .unwrap();
            assert_eq!(resolved.language.as_deref(), Some("he"));
            assert_eq!(
                store
                    .with_current(|log| log.stt_language_hint.clone())
                    .flatten()
                    .as_deref(),
                Some("he")
            );
            assert_eq!(inner.config.stt_language.as_deref(), Some("en"));
            assert_eq!(profile.stt_language.as_deref(), Some("yi"));
        }
    }

    #[test]
    fn forced_provider_and_model_override_profile_and_preset_settings() {
        let mut inner = PipelineInner::new(PipelineConfig {
            stt_provider: "groq".to_string(),
            stt_language: Some("en".to_string()),
            ..Default::default()
        });
        insert_cached_fake_provider(
            &mut inner,
            "openai",
            Some("forced-model".to_string()),
            Some("es".to_string()),
        );

        let profile = profile_with_stt(Some("deepgram"), Some("profile-model"), Some("fr"), None);
        let preset = preset_with_stt(Some("groq"), Some("preset-model"), Some("es"), None);

        let resolved = resolve_stt_provider_for_transcription(
            &mut inner,
            SttProviderResolutionRequest {
                active_profile: Some(&profile),
                active_preset: Some(&preset),
                forced_provider: Some(" OpenAI "),
                forced_model: Some(" forced-model "),
                forced_language: None,
            },
        )
        .expect("forced provider should resolve from cache");

        assert_eq!(resolved.provider_id, "openai");
        assert_eq!(resolved.model.as_deref(), Some("forced-model"));
        assert_eq!(resolved.language.as_deref(), Some("es"));
    }

    #[test]
    fn profile_provider_failure_preserves_selection_instead_of_using_global_provider() {
        let store = RequestLogStore::new();
        store.start_request("initial".to_string(), None);

        let mut inner = PipelineInner::new(PipelineConfig {
            stt_provider: "groq".to_string(),
            stt_language: Some("en".to_string()),
            request_log_store: Some(store.clone()),
            ..Default::default()
        });
        insert_cached_fake_provider(&mut inner, "groq", None, Some("en".to_string()));

        let profile = profile_with_stt(Some("openai"), None, Some("en"), None);
        let result = resolve_stt_provider_for_transcription(
            &mut inner,
            SttProviderResolutionRequest {
                active_profile: Some(&profile),
                active_preset: None,
                forced_provider: None,
                forced_model: None,
                forced_language: None,
            },
        );
        assert!(result.is_err());
        let logged_provider = store.with_current(|log| log.stt_provider.clone());
        assert_eq!(logged_provider.as_deref(), Some("openai"));
    }

    #[test]
    fn managed_runtime_fallback_reports_and_logs_actual_provider() {
        let store = RequestLogStore::new();
        store.start_request("initial".to_string(), None);

        let mut inner = PipelineInner::new(PipelineConfig {
            stt_provider: "groq".to_string(),
            stt_model: Some("whisper-large-v3-turbo".to_string()),
            stt_language: Some("en".to_string()),
            managed_inference_enabled: true,
            managed_inference_fallback_stt_provider: Some("assemblyai".to_string()),
            request_log_store: Some(store.clone()),
            ..Default::default()
        });
        insert_cached_fake_provider(
            &mut inner,
            "assemblyai",
            Some("whisper-large-v3-turbo".to_string()),
            Some("en".to_string()),
        );

        let resolved = resolve_stt_provider_for_transcription(
            &mut inner,
            SttProviderResolutionRequest {
                active_profile: None,
                active_preset: None,
                forced_provider: None,
                forced_model: None,
                forced_language: None,
            },
        )
        .expect("managed fallback provider should resolve from cache");

        assert_eq!(resolved.provider_id, "assemblyai");
        let logged = store.with_current(|log| (log.stt_provider.clone(), log.managed_inference));
        assert_eq!(logged, Some(("assemblyai".to_string(), false)));
    }
}
