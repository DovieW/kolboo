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
    canonicalize_stt_provider_id, managed_gateway_ready, resolve_stt_provider_for_runtime,
    stt_provider, PipelineError, PipelineInner,
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
}

/// Fully-resolved STT provider details for a transcription attempt.
pub(super) struct ResolvedSttProvider {
    pub provider: Arc<dyn SttProvider>,
    pub provider_id: String,
    pub model: Option<String>,
    pub language: Option<String>,
    pub timeout: Duration,
}

/// Resolve effective STT settings, apply per-run overrides, update request-log
/// provider/model fields, create the provider, and fall back to the global STT
/// provider if a profile/override provider is unavailable.
pub(super) fn resolve_stt_provider_for_transcription(
    inner: &mut PipelineInner,
    request: SttProviderResolutionRequest<'_>,
) -> Result<ResolvedSttProvider, PipelineError> {
    let mut effective =
        inner.resolve_effective_stt_settings(request.active_profile, request.active_preset);

    apply_forced_overrides(
        &mut effective.provider_id,
        &mut effective.model,
        request.forced_provider,
        request.forced_model,
    );

    let mut provider_id_used = effective.provider_id.clone();
    let mut model_used = model_for_log(inner, provider_id_used.as_str(), effective.model.clone());
    let mut language_used = effective.language.clone();

    log_stt_provider_selection(inner, provider_id_used.as_str(), model_used.clone());

    let provider = match get_or_create_stt_provider(
        inner,
        &effective.provider_id,
        effective.model.clone(),
        effective.language.clone(),
    ) {
        Ok(provider) => provider,
        Err(err) => {
            let global_provider = canonicalize_stt_provider_id(&inner.config.stt_provider);
            if global_provider == effective.provider_id {
                inner.set_error(&format!("STT provider init failed: {}", err));
                return Err(err);
            }

            log::warn!(
                "Pipeline: Effective STT provider '{}' unavailable ({}), falling back to '{}'",
                effective.provider_id,
                err,
                global_provider
            );

            let global_model = inner.config.stt_model.clone();
            let global_language = inner.config.stt_language.clone();

            provider_id_used = global_provider.clone();
            model_used = model_for_log(inner, provider_id_used.as_str(), global_model.clone());
            language_used = global_language.clone();

            log_stt_provider_selection(inner, provider_id_used.as_str(), model_used.clone());

            get_or_create_stt_provider(inner, &global_provider, global_model, global_language)
                .map_err(|fallback_err| {
                    inner.set_error(&format!("No STT provider configured: {}", fallback_err));
                    fallback_err
                })?
        }
    };

    Ok(ResolvedSttProvider {
        provider,
        provider_id: provider_id_used,
        model: model_used,
        language: language_used,
        timeout: effective.timeout,
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

fn log_stt_provider_selection(inner: &PipelineInner, provider_id: &str, model: Option<String>) {
    if let Some(store) = inner.config.request_log_store.as_ref() {
        store.with_current(|log| {
            log.stt_provider = provider_id.to_string();
            log.stt_model = model;
        });
    }
}

pub(super) fn stt_provider_cache_key(
    inner: &PipelineInner,
    provider_id: &str,
    model: Option<String>,
    language: Option<String>,
) -> String {
    // NOTE: for Local Whisper, the "model" setting is not meaningful (Whisper model is
    // selected via `whisper_model_path`). Using the global `stt_model` here can cause
    // unnecessary cache misses and, worse, repeated expensive model loads.
    let language_key = language.unwrap_or_else(|| "<auto>".to_string());
    let model_key = if provider_id == "local-whisper" {
        inner.local_whisper_model_key_for_cache()
    } else {
        model.unwrap_or_else(|| "<default>".to_string())
    };

    format!(
        "{}::{}::{}::live={}",
        provider_id, model_key, language_key, inner.config.stt_live_output
    )
}

pub(super) fn get_or_create_stt_provider(
    inner: &mut PipelineInner,
    provider_id: &str,
    model: Option<String>,
    language: Option<String>,
) -> Result<Arc<dyn SttProvider>, PipelineError> {
    let provider_id = resolve_stt_provider_for_runtime(&inner.config, provider_id);
    let managed_ready =
        inner.config.managed_inference_enabled && managed_gateway_ready(&inner.config);
    // Local providers never use managed transport.
    let managed_transport_active =
        managed_ready && provider_id != "local-whisper" && provider_id != "whisper-server";

    if managed_transport_active {
        if let Some(store) = &inner.config.request_log_store {
            let _ = store.with_current(|log| {
                log.managed_inference = true;
            });
        }
    }

    let cache_key =
        stt_provider_cache_key(inner, provider_id.as_str(), model.clone(), language.clone());

    if let Some(provider) = inner.stt_provider_cache.get(&cache_key) {
        return Ok(provider.clone());
    }

    // Manual local-whisper mode: require explicit preload to avoid surprise UI stalls
    // during stop/transcribe.
    if provider_id == "local-whisper" && inner.config.local_whisper_load_mode == "manual" {
        return Err(PipelineError::Config(
            "Local Whisper is set to Manual load. Click 'Load model' in Settings (or switch load mode to 'On transcribe').".to_string(),
        ));
    }

    #[cfg(feature = "local-whisper")]
    if provider_id == "local-whisper" {
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
            return Ok(provider);
        }

        return Err(PipelineError::Config(
            "Local Whisper selected but no model path configured".to_string(),
        ));
    }

    if provider_id == "whisper-server" {
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
        return Ok(provider);
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
            .get(&provider_id)
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
            managed_gateway_url: if managed_transport_active {
                inner.config.managed_inference_gateway_url.clone()
            } else {
                None
            },
            transcription_prompt: inner.config.stt_transcription_prompt.clone(),
            request_log_store: inner.config.request_log_store.clone(),
            stt_live_output: inner.config.stt_live_output,
        },
    )?;

    inner.stt_provider_cache.insert(cache_key, provider.clone());
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::apply_forced_overrides;

    #[test]
    fn forced_provider_is_trimmed_and_canonicalized() {
        let mut provider_id = "openai".to_string();
        let mut model = Some("base".to_string());

        apply_forced_overrides(&mut provider_id, &mut model, Some(" Groq "), None);

        assert_eq!(provider_id, "groq");
        assert_eq!(model.as_deref(), Some("base"));
    }

    #[test]
    fn blank_forced_model_clears_model_override() {
        let mut provider_id = "openai".to_string();
        let mut model = Some("whisper-1".to_string());

        apply_forced_overrides(&mut provider_id, &mut model, None, Some("   "));

        assert_eq!(provider_id, "openai");
        assert_eq!(model, None);
    }
}
