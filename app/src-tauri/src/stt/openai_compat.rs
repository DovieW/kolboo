//! Shared helpers for OpenAI-compatible transcription endpoints.
//!
//! Several STT providers accept Whisper-style multipart form uploads using the
//! same shape:
//! - multipart/form-data
//! - fields: `file` (audio/wav) + `model` + optional `prompt`

use super::SttError;
use crate::request_log::RequestLogStore;
use reqwest::multipart;
use serde_json::json;

pub(super) fn normalize_optional_text(s: Option<&str>) -> Option<String> {
    let s = s.map(str::trim).filter(|v| !v.is_empty())?;
    Some(s.to_string())
}

pub(super) fn wav_transcription_form(
    audio: &[u8],
    model: &str,
    prompt: Option<&str>,
    language: Option<&str>,
) -> Result<multipart::Form, SttError> {
    let part = multipart::Part::bytes(audio.to_vec())
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| SttError::Audio(format!("Failed to create multipart: {}", e)))?;

    let mut form = multipart::Form::new()
        .part("file", part)
        .text("model", model.to_string());

    if let Some(prompt) = normalize_optional_text(prompt) {
        form = form.text("prompt", prompt);
    }

    if let Some(language) = normalize_optional_text(language) {
        form = form.text("language", language);
    }

    Ok(form)
}

// This is a shared helper used by multiple providers; keeping parameters explicit makes
// the call sites easier to read.
#[allow(clippy::too_many_arguments)]
pub(super) async fn transcribe_wav_multipart_openai_compat<BuildRequest, MapNetworkError>(
    client: &reqwest::Client,
    provider: &'static str,
    api_error_label: &'static str,
    endpoint: &str,
    audio: &[u8],
    model: &str,
    prompt: Option<&str>,
    language: Option<&str>,
    request_log_store: Option<&RequestLogStore>,
    build_request: BuildRequest,
    map_network_error: MapNetworkError,
) -> Result<String, SttError>
where
    BuildRequest: FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
    MapNetworkError: FnOnce(reqwest::Error) -> SttError,
{
    if let Some(store) = request_log_store {
        let request_json = json!({
            "provider": provider,
            "endpoint": endpoint,
            "content_type": "multipart/form-data",
            "fields": {
                "model": model,
                "prompt": prompt,
                "language": language,
            },
            "file": {
                "name": "audio.wav",
                "mime": "audio/wav",
                "bytes": audio.len(),
                "data": "<binary audio omitted>",
            }
        });

        store.with_current(|log| {
            log.stt_request_json = Some(request_json);
        });
    }

    let form = wav_transcription_form(audio, model, prompt, language)?;

    let response = crate::http::with_cloudflare_access_headers_if_target(
        build_request(client.post(endpoint)),
        endpoint,
    )
    .multipart(form)
    .send()
    .await
    .map_err(|e| {
        if e.is_timeout() {
            SttError::Timeout
        } else {
            map_network_error(e)
        }
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(SttError::Api(format!(
            "{} ({}): {}",
            api_error_label, status, error_text
        )));
    }

    let result: serde_json::Value = response.json().await?;

    if let Some(store) = request_log_store {
        let result_for_log = result.clone();
        store.with_current(|log| {
            log.stt_response_json = Some(result_for_log);
        });
    }

    Ok(result
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_optional_text_trims_and_filters_empty() {
        assert_eq!(normalize_optional_text(None), None);
        assert_eq!(normalize_optional_text(Some("")), None);
        assert_eq!(normalize_optional_text(Some("   ")), None);
        assert_eq!(
            normalize_optional_text(Some("  hi  ")),
            Some("hi".to_string())
        );
    }
}
