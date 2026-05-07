//! Cloud STT provider construction adapters.
//!
//! STT Provider Resolution decides *which* provider should be used. This Module
//! keeps provider-specific construction local so resolver code does not need to
//! know every constructor quirk (managed base URLs, live-output VAD, prompt
//! support, and request-log wiring).

use std::sync::Arc;

use reqwest::Client;

use crate::request_log::RequestLogStore;
use crate::settings::ProxySettings;
use crate::stt::SttProvider;

pub(super) struct CloudSttProviderBuildContext {
    pub client: Client,
    pub model: Option<String>,
    pub language: Option<String>,
    pub api_key: String,
    pub proxy_settings: ProxySettings,
    pub managed_gateway_url: Option<String>,
    pub transcription_prompt: Option<String>,
    pub request_log_store: Option<RequestLogStore>,
    pub stt_live_output: bool,
}

pub(super) struct CloudSttProviderAdapter {
    #[cfg_attr(not(test), allow(dead_code))]
    pub id: &'static str,
    build: fn(CloudSttProviderBuildContext) -> Arc<dyn SttProvider>,
}

impl CloudSttProviderAdapter {
    pub fn build(self, ctx: CloudSttProviderBuildContext) -> Arc<dyn SttProvider> {
        (self.build)(ctx)
    }
}

pub(super) fn adapter_for(provider_id: &str) -> Option<CloudSttProviderAdapter> {
    let adapter = match provider_id {
        "openai" => CloudSttProviderAdapter {
            id: "openai",
            build: build_openai,
        },
        "fireworks" => CloudSttProviderAdapter {
            id: "fireworks",
            build: build_fireworks,
        },
        "aquavoice" => CloudSttProviderAdapter {
            id: "aquavoice",
            build: build_aquavoice,
        },
        "groq" => CloudSttProviderAdapter {
            id: "groq",
            build: build_groq,
        },
        "elevenlabs" => CloudSttProviderAdapter {
            id: "elevenlabs",
            build: build_elevenlabs,
        },
        "assemblyai" => CloudSttProviderAdapter {
            id: "assemblyai",
            build: build_assemblyai,
        },
        "speechmatics" => CloudSttProviderAdapter {
            id: "speechmatics",
            build: build_speechmatics,
        },
        "deepgram" => CloudSttProviderAdapter {
            id: "deepgram",
            build: build_deepgram,
        },
        _ => return None,
    };

    Some(adapter)
}

#[cfg(test)]
fn supported_provider_ids() -> &'static [&'static str] {
    &[
        "openai",
        "fireworks",
        "aquavoice",
        "groq",
        "elevenlabs",
        "assemblyai",
        "speechmatics",
        "deepgram",
    ]
}

fn build_openai(ctx: CloudSttProviderBuildContext) -> Arc<dyn SttProvider> {
    let CloudSttProviderBuildContext {
        client,
        model,
        language,
        api_key,
        proxy_settings,
        managed_gateway_url,
        transcription_prompt,
        request_log_store,
        ..
    } = ctx;

    let provider = crate::stt::OpenAiSttProvider::with_client(
        client,
        api_key,
        model,
        language,
        transcription_prompt,
    );
    let provider = if let Some(url) = managed_gateway_url {
        provider.with_api_base_url(url)
    } else {
        provider
    };

    Arc::new(
        provider
            .with_proxy_settings(proxy_settings)
            .with_request_log_store(request_log_store),
    )
}

fn build_fireworks(ctx: CloudSttProviderBuildContext) -> Arc<dyn SttProvider> {
    let CloudSttProviderBuildContext {
        client,
        model,
        language,
        api_key,
        proxy_settings,
        managed_gateway_url,
        transcription_prompt,
        request_log_store,
        ..
    } = ctx;

    let provider = crate::stt::FireworksSttProvider::with_client(
        client,
        api_key,
        model,
        language,
        transcription_prompt,
    );
    let provider = if let Some(url) = managed_gateway_url {
        provider.with_api_base_url(url)
    } else {
        provider
    };

    Arc::new(
        provider
            .with_proxy_settings(proxy_settings)
            .with_request_log_store(request_log_store),
    )
}

fn build_aquavoice(ctx: CloudSttProviderBuildContext) -> Arc<dyn SttProvider> {
    let CloudSttProviderBuildContext {
        client,
        model,
        language,
        api_key,
        managed_gateway_url,
        transcription_prompt,
        request_log_store,
        ..
    } = ctx;

    let provider = crate::stt::AquavoiceSttProvider::with_client(
        client,
        api_key,
        model,
        language,
        transcription_prompt,
    );
    let provider = if let Some(url) = managed_gateway_url {
        provider.with_api_base_url(url)
    } else {
        provider
    };

    Arc::new(provider.with_request_log_store(request_log_store))
}

fn build_groq(ctx: CloudSttProviderBuildContext) -> Arc<dyn SttProvider> {
    let CloudSttProviderBuildContext {
        client,
        model,
        language,
        api_key,
        managed_gateway_url,
        transcription_prompt,
        request_log_store,
        ..
    } = ctx;

    let provider = crate::stt::GroqSttProvider::with_client(
        client,
        api_key,
        model,
        language,
        transcription_prompt,
    );
    let provider = if let Some(url) = managed_gateway_url {
        provider.with_api_base_url(url)
    } else {
        provider
    };

    Arc::new(provider.with_request_log_store(request_log_store))
}

fn build_elevenlabs(ctx: CloudSttProviderBuildContext) -> Arc<dyn SttProvider> {
    let CloudSttProviderBuildContext {
        client,
        model,
        language,
        api_key,
        proxy_settings,
        managed_gateway_url,
        request_log_store,
        stt_live_output,
        ..
    } = ctx;

    let provider = crate::stt::ElevenLabsSttProvider::with_client(client, api_key, model, language)
        .with_vad_commit(stt_live_output);
    let provider = if let Some(url) = managed_gateway_url {
        provider.with_api_base_url(url)
    } else {
        provider
    };

    Arc::new(
        provider
            .with_proxy_settings(proxy_settings)
            .with_request_log_store(request_log_store),
    )
}

fn build_assemblyai(ctx: CloudSttProviderBuildContext) -> Arc<dyn SttProvider> {
    let CloudSttProviderBuildContext {
        client,
        model,
        language,
        api_key,
        proxy_settings,
        managed_gateway_url,
        request_log_store,
        ..
    } = ctx;

    let provider = crate::stt::AssemblyAiSttProvider::with_client(client, api_key, model, language);
    let provider = if let Some(url) = managed_gateway_url {
        provider.with_api_base_url(url)
    } else {
        provider
    };

    Arc::new(
        provider
            .with_proxy_settings(proxy_settings)
            .with_request_log_store(request_log_store),
    )
}

fn build_speechmatics(ctx: CloudSttProviderBuildContext) -> Arc<dyn SttProvider> {
    let CloudSttProviderBuildContext {
        model,
        language,
        api_key,
        proxy_settings,
        request_log_store,
        ..
    } = ctx;

    Arc::new(
        crate::stt::SpeechmaticsSttProvider::new(api_key, model, language)
            .with_proxy_settings(proxy_settings)
            .with_request_log_store(request_log_store),
    )
}

fn build_deepgram(ctx: CloudSttProviderBuildContext) -> Arc<dyn SttProvider> {
    let CloudSttProviderBuildContext {
        client,
        model,
        language,
        api_key,
        proxy_settings,
        managed_gateway_url,
        request_log_store,
        ..
    } = ctx;

    let provider = crate::stt::DeepgramSttProvider::with_client(client, api_key, model, language);
    let provider = if let Some(url) = managed_gateway_url {
        provider.with_api_base_url(url)
    } else {
        provider
    };

    Arc::new(
        provider
            .with_proxy_settings(proxy_settings)
            .with_request_log_store(request_log_store),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn test_context() -> CloudSttProviderBuildContext {
        CloudSttProviderBuildContext {
            client: reqwest::Client::new(),
            model: Some("test-model".to_string()),
            language: Some("en".to_string()),
            api_key: "test-key".to_string(),
            proxy_settings: ProxySettings::default(),
            managed_gateway_url: Some("https://managed.example.test".to_string()),
            transcription_prompt: Some("keep punctuation".to_string()),
            request_log_store: None,
            stt_live_output: true,
        }
    }

    #[test]
    fn adapter_registry_has_expected_provider_ids_without_duplicates() {
        let ids = supported_provider_ids();
        let unique: HashSet<&str> = ids.iter().copied().collect();

        assert_eq!(ids.len(), unique.len());
        for id in ids {
            assert_eq!(adapter_for(id).map(|adapter| adapter.id), Some(*id));
        }
    }

    #[test]
    fn adapters_construct_multiple_concrete_providers_without_network_calls() {
        // Two concrete adapters keep this from becoming a hypothetical seam. The
        // deletion test: removing this registry would push constructor quirks back
        // into the STT Provider Resolution path.
        let openai = adapter_for("openai")
            .expect("openai adapter")
            .build(test_context());
        let groq = adapter_for("groq")
            .expect("groq adapter")
            .build(test_context());

        assert_eq!(openai.name(), "openai");
        assert_eq!(groq.name(), "groq");
    }
}
