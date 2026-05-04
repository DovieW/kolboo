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

// STT requests can legitimately take a while (slow networks, provider backlog, long audio).
// We intentionally keep the reqwest client timeout very generous so the *user-configured*
// Tokio timeout in `stt_flow` is the effective source of truth.
const STT_HTTP_CLIENT_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Parameters needed to create an STT provider.
pub(crate) struct SttProviderParams {
    pub provider_id: String,
    pub model: Option<String>,
    pub language: Option<String>,
    pub api_key: String,
    pub managed_gateway_url: Option<String>,
    pub transcription_prompt: Option<String>,
    pub request_log_store: Option<RequestLogStore>,
    /// When true, streaming providers should use server-side VAD to auto-commit
    /// speech segments during recording (enables live output).
    pub stt_live_output: bool,
}

/// Create an HTTP client configured for STT requests.
pub(crate) fn build_stt_client(proxy_settings: &ProxySettings) -> Result<Client, PipelineError> {
    crate::network::build_http_client_with_timeout(proxy_settings, STT_HTTP_CLIENT_TIMEOUT)
        .map_err(|e| PipelineError::Config(format!("Failed to create HTTP client: {}", e)))
}

/// Create an STT provider based on the provider ID and parameters.
///
/// Note: This does NOT handle local-whisper or whisper-server. Local-provider
/// lifecycle/readiness decisions live in `pipeline::local_provider_lifecycle`,
/// and provider-specific construction remains in STT provider resolution.
pub(crate) fn create_cloud_stt_provider(
    client: Client,
    params: SttProviderParams,
) -> Result<Arc<dyn SttProvider>, PipelineError> {
    let SttProviderParams {
        provider_id,
        model,
        language,
        api_key,
        managed_gateway_url,
        transcription_prompt,
        request_log_store,
        stt_live_output,
    } = params;

    if api_key.trim().is_empty() {
        return Err(PipelineError::Config(format!(
            "STT provider '{}' requires an API key",
            provider_id
        )));
    }

    let managed_gateway_url = managed_gateway_url
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty());

    let provider: Arc<dyn SttProvider> = match provider_id.as_str() {
        "openai" => {
            let provider = crate::stt::OpenAiSttProvider::with_client(
                client,
                api_key,
                model,
                language,
                transcription_prompt,
            );
            let provider = if let Some(url) = managed_gateway_url.clone() {
                provider.with_api_base_url(url)
            } else {
                provider
            };

            Arc::new(provider.with_request_log_store(request_log_store))
        }
        "fireworks" => {
            let provider = crate::stt::FireworksSttProvider::with_client(
                client,
                api_key,
                model,
                language,
                transcription_prompt,
            );
            let provider = if let Some(url) = managed_gateway_url.clone() {
                provider.with_api_base_url(url)
            } else {
                provider
            };

            Arc::new(provider.with_request_log_store(request_log_store))
        }
        "aquavoice" => {
            let provider = crate::stt::AquavoiceSttProvider::with_client(
                client,
                api_key,
                model,
                language,
                transcription_prompt,
            );
            let provider = if let Some(url) = managed_gateway_url.clone() {
                provider.with_api_base_url(url)
            } else {
                provider
            };

            Arc::new(provider.with_request_log_store(request_log_store))
        }
        "groq" => {
            let provider = crate::stt::GroqSttProvider::with_client(
                client,
                api_key,
                model,
                language,
                transcription_prompt,
            );
            let provider = if let Some(url) = managed_gateway_url.clone() {
                provider.with_api_base_url(url)
            } else {
                provider
            };

            Arc::new(provider.with_request_log_store(request_log_store))
        }
        "elevenlabs" => {
            let provider =
                crate::stt::ElevenLabsSttProvider::with_client(client, api_key, model, language)
                    .with_vad_commit(stt_live_output);
            let provider = if let Some(url) = managed_gateway_url.clone() {
                provider.with_api_base_url(url)
            } else {
                provider
            };
            Arc::new(provider.with_request_log_store(request_log_store))
        }
        "assemblyai" => {
            let provider =
                crate::stt::AssemblyAiSttProvider::with_client(client, api_key, model, language);
            let provider = if let Some(url) = managed_gateway_url.clone() {
                provider.with_api_base_url(url)
            } else {
                provider
            };
            Arc::new(provider.with_request_log_store(request_log_store))
        }
        "speechmatics" => Arc::new(
            crate::stt::SpeechmaticsSttProvider::new(api_key, model, language)
                .with_request_log_store(request_log_store),
        ),
        "deepgram" => {
            let provider =
                crate::stt::DeepgramSttProvider::with_client(client, api_key, model, language);
            let provider = if let Some(url) = managed_gateway_url.clone() {
                provider.with_api_base_url(url)
            } else {
                provider
            };
            Arc::new(provider.with_request_log_store(request_log_store))
        }
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
#[allow(dead_code)]
pub(super) fn default_timeout_for_provider(provider_id: &str) -> Duration {
    match provider_id {
        "openai" | "fireworks" | "assemblyai" => Duration::from_secs(120),
        "aquavoice" | "groq" | "elevenlabs" | "deepgram" => Duration::from_secs(60),
        _ => Duration::from_secs(60),
    }
}
