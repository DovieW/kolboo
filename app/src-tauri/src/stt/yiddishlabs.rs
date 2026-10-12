//! YiddishLabs' two distinct protocols share a provider identity, not credentials.
//! Standard dispatch/checkpoint ownership lives in `yiddishlabs/jobs.rs`; normal
//! dictation must never enter its paid asynchronous submission path.
use super::{AudioFormat, SttError, SttProvider, SttTranscript, SttUploadContext};
use async_trait::async_trait;
use reqwest::{multipart, Client};
use serde_json::Value;
use std::time::Duration;

pub(crate) const FLASH_MODEL: &str = "yiddishlabs-3-turbo";
pub(crate) const STANDARD_MODEL: &str = "yiddishlabs-standard";
pub(crate) const FLASH_KEY_SLOT: &str = "yiddishlabs_flash";
pub(crate) const STANDARD_KEY_SLOT: &str = "yiddishlabs_standard";
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
mod jobs;
pub(crate) use jobs::{checkpoint_path, delete_owned_checkpoints};

#[derive(Clone)]
pub struct YiddishLabsSttProvider {
    client: Client,
    api_key: String,
    model: String,
    language: Option<String>,
    prompt: Option<String>,
    managed_gateway: Option<String>,
    flash_endpoint: String,
    standard_endpoint: String,
}

impl YiddishLabsSttProvider {
    pub(crate) fn with_client(
        client: Client,
        api_key: String,
        model: Option<String>,
        language: Option<String>,
        prompt: Option<String>,
        managed_gateway: Option<String>,
    ) -> Self {
        Self {
            client,
            api_key,
            model: model.unwrap_or_else(|| FLASH_MODEL.into()),
            language,
            prompt,
            managed_gateway,
            flash_endpoint: "https://flash.yiddishlabs.com/v1/audio/transcriptions".into(),
            standard_endpoint: "https://app.yiddishlabs.com/api/v1/transcriptions".into(),
        }
    }
}

pub(crate) fn language_hint(value: Option<&str>, standard: bool) -> Result<&str, SttError> {
    let value = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("auto");
    if matches!(value, "auto" | "yi" | "en" | "he") || (standard && value == "lk") {
        Ok(value)
    } else {
        Err(SttError::Config(
            "This language hint is not supported by the selected YiddishLabs model".into(),
        ))
    }
}

pub(crate) async fn response_json(mut response: reqwest::Response) -> Result<Value, SttError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        SttError::Api("YiddishLabs' response could not be read; the recording is retained".into())
    })? {
        if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(SttError::Api(
                "YiddishLabs returned an oversized response; the recording is retained".into(),
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| {
        SttError::Api("YiddishLabs returned an invalid response; the recording is retained".into())
    })
}

#[async_trait]
impl SttProvider for YiddishLabsSttProvider {
    fn with_renewed_managed_token(&self, token: &str) -> Option<std::sync::Arc<dyn SttProvider>> {
        if self.managed_gateway.is_none() || token.trim().is_empty() {
            return None;
        }
        let mut renewed = self.clone();
        renewed.api_key = token.into();
        Some(std::sync::Arc::new(renewed))
    }
    async fn transcribe(&self, audio: &[u8], _format: &AudioFormat) -> Result<String, SttError> {
        if self.model != FLASH_MODEL {
            return Err(SttError::Config(if self.model == STANDARD_MODEL {
                "YiddishLabs Standard is available only for imported files and meetings. The provider stores submitted content.".into()
            } else {
                "Unknown YiddishLabs model; your selection has not changed".into()
            }));
        }
        if self.api_key.trim().is_empty() {
            return Err(SttError::Config(
                "Add the YiddishLabs Flash API key in secure key settings".into(),
            ));
        }
        if audio.is_empty() || audio.len() > 100 * 1024 * 1024 {
            return Err(SttError::Audio(
                "YiddishLabs Flash accepts a nonempty upload of at most 100 MB".into(),
            ));
        }
        let language = language_hint(self.language.as_deref(), false)?;
        let mut form = multipart::Form::new()
            .part(
                "file",
                multipart::Part::bytes(audio.to_vec()).file_name("recording.wav"),
            )
            .text("model", FLASH_MODEL)
            .text("language", language.to_owned())
            .text("response_format", "json");
        if let Some(prompt) = self.prompt.as_ref().filter(|prompt| !prompt.is_empty()) {
            if prompt.len() > 8000 {
                return Err(SttError::Config(
                    "The YiddishLabs transcription context is too long".into(),
                ));
            }
            form = form.text("prompt", prompt.clone());
        }
        let endpoint = self
            .managed_gateway
            .as_ref()
            .map(|gateway| crate::http::join_base_url(gateway, "/v1/audio/transcriptions"))
            .unwrap_or_else(|| self.flash_endpoint.clone());
        let mut request = self
            .client
            .post(&endpoint)
            .bearer_auth(&self.api_key)
            .timeout(Duration::from_secs(120));
        if self.managed_gateway.is_some() {
            request = request
                .header("x-kolboo-model-provider", "yiddishlabs")
                .header("x-idempotency-key", uuid::Uuid::new_v4().to_string());
        }
        let response = crate::http::with_cloudflare_access_headers_if_target(request, &endpoint)
            .multipart(form)
            .send()
            .await
            .map_err(|_| {
                SttError::Api(
                    "YiddishLabs processing is unavailable; the recording is retained".into(),
                )
            })?;
        if !response.status().is_success() {
            return Err(SttError::Api(format!(
                "YiddishLabs Flash rejected the upload (HTTP {}); the recording is retained",
                response.status().as_u16()
            )));
        }
        let value = response_json(response).await?;
        value
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                SttError::Api(
                    "YiddishLabs returned no transcript; the recording is retained".into(),
                )
            })
    }
    fn name(&self) -> &'static str {
        "yiddishlabs"
    }

    async fn transcribe_upload(
        &self,
        audio: &[u8],
        format: &AudioFormat,
        context: &SttUploadContext,
    ) -> Result<SttTranscript, SttError> {
        if self.model != STANDARD_MODEL {
            return self.transcribe_detailed(audio, format).await;
        }
        Ok(SttTranscript {
            text: jobs::transcribe(self, audio, context).await?,
            segments: Vec::new(),
        })
    }
}

#[cfg(test)]
#[path = "yiddishlabs/tests.rs"]
mod tests;
