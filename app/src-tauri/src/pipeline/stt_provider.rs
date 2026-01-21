//! STT provider construction and configuration.
//!
//! This module provides utilities for creating and configuring STT providers
//! based on pipeline configuration.

use crate::pipeline::PipelineError;
use crate::request_log::RequestLogStore;
use crate::settings::ProxySettings;
use crate::stt::SttProvider;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;

/// Parameters needed to create an STT provider.
pub(super) struct SttProviderParams {
    pub provider_id: String,
    pub model: Option<String>,
    pub api_key: String,
    pub transcription_prompt: Option<String>,
    pub request_log_store: Option<RequestLogStore>,
}

/// Create an HTTP client configured for STT requests.
pub(super) fn build_stt_client(
    proxy_settings: &ProxySettings,
    timeout: Duration,
) -> Result<Client, PipelineError> {
    crate::network::build_http_client_with_timeout(proxy_settings, timeout)
        .map_err(|e| PipelineError::Config(format!("Failed to create HTTP client: {}", e)))
}

/// Create an STT provider based on the provider ID and parameters.
///
/// Note: This does NOT handle local-whisper or whisper-server, which have special
/// configuration requirements (model paths, base URLs). Those are handled separately
/// in `PipelineInner::get_or_create_stt_provider`.
pub(super) fn create_cloud_stt_provider(
    client: Client,
    params: SttProviderParams,
) -> Result<Arc<dyn SttProvider>, PipelineError> {
    let SttProviderParams {
        provider_id,
        model,
        api_key,
        transcription_prompt,
        request_log_store,
    } = params;

    if api_key.is_empty() {
        return Err(PipelineError::Config(format!(
            "STT provider '{}' requires an API key",
            provider_id
        )));
    }

    let provider: Arc<dyn SttProvider> = match provider_id.as_str() {
        "openai" => Arc::new(
            crate::stt::OpenAiSttProvider::with_client(
                client,
                api_key,
                model,
                transcription_prompt,
            )
            .with_request_log_store(request_log_store),
        ),
        "fireworks" => Arc::new(
            crate::stt::FireworksSttProvider::with_client(
                client,
                api_key,
                model,
                transcription_prompt,
            )
            .with_request_log_store(request_log_store),
        ),
        "aquavoice" => Arc::new(
            crate::stt::AquavoiceSttProvider::with_client(
                client,
                api_key,
                model,
                transcription_prompt,
            )
            .with_request_log_store(request_log_store),
        ),
        "groq" => Arc::new(
            crate::stt::GroqSttProvider::with_client(client, api_key, model, transcription_prompt)
                .with_request_log_store(request_log_store),
        ),
        "elevenlabs" => Arc::new(
            crate::stt::ElevenLabsSttProvider::with_client(client, api_key, model)
                .with_request_log_store(request_log_store),
        ),
        "assemblyai" => Arc::new(
            crate::stt::AssemblyAiSttProvider::with_client(client, api_key, model)
                .with_request_log_store(request_log_store),
        ),
        "speechmatics" => Arc::new(
            crate::stt::SpeechmaticsSttProvider::new(api_key, model)
                .with_request_log_store(request_log_store),
        ),
        "deepgram" => Arc::new(
            crate::stt::DeepgramSttProvider::with_client(client, api_key, model)
                .with_request_log_store(request_log_store),
        ),
        other => {
            return Err(PipelineError::Config(format!(
                "Unknown STT provider: {}",
                other
            )))
        }
    };

    Ok(provider)
}

/// Get the default timeout for a given STT provider.
pub(super) fn default_timeout_for_provider(provider_id: &str) -> Duration {
    match provider_id {
        "openai" | "fireworks" | "assemblyai" => Duration::from_secs(120),
        "aquavoice" | "groq" | "elevenlabs" | "deepgram" => Duration::from_secs(60),
        _ => Duration::from_secs(60),
    }
}
