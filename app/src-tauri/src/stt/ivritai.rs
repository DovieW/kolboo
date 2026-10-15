//! Managed-only Hebrew transcription. The origin credential never enters desktop.
use super::{openai_compat, AudioFormat, SttError, SttProvider};
use async_trait::async_trait;
use std::sync::Arc;

pub(crate) const MODEL: &str = "ivrit-ai/whisper-large-v3-turbo";

#[derive(Clone)]
pub(crate) struct IvritAiSttProvider {
    pub client: reqwest::Client,
    pub gateway: String,
    pub token: String,
    pub prompt: Option<String>,
}

#[async_trait]
impl SttProvider for IvritAiSttProvider {
    fn name(&self) -> &'static str {
        "ivritai"
    }

    fn with_renewed_managed_token(&self, token: &str) -> Option<Arc<dyn SttProvider>> {
        if token.trim().is_empty() {
            return None;
        }
        let mut renewed = self.clone();
        renewed.token = token.into();
        Some(Arc::new(renewed))
    }

    async fn transcribe(&self, audio: &[u8], _format: &AudioFormat) -> Result<String, SttError> {
        let endpoint = format!(
            "{}/v1/audio/transcriptions",
            self.gateway.trim_end_matches('/')
        );
        openai_compat::transcribe_wav_multipart_openai_compat(
            &self.client,
            "ivritai",
            "ivritAI Managed transcription error",
            &endpoint,
            audio,
            MODEL,
            self.prompt.as_deref(),
            // Selecting this Hebrew-only model determines its language. Never
            // forward an inherited multilingual-model hint or an old snapshot.
            Some("he"),
            None,
            |request| {
                openai_compat::with_managed_selection(
                    request.bearer_auth(&self.token),
                    Some("ivritai"),
                )
            },
            SttError::Network,
        )
        .await
    }
}
