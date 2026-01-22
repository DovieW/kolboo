//! Shared helpers for OpenAI-compatible transcription endpoints.
//!
//! Several STT providers accept Whisper-style multipart form uploads using the
//! same shape:
//! - multipart/form-data
//! - fields: `file` (audio/wav) + `model` + optional `prompt`

use super::SttError;
use reqwest::multipart;

pub(super) fn normalize_optional_text(s: Option<&str>) -> Option<String> {
    let s = s.map(str::trim).filter(|v| !v.is_empty())?;
    Some(s.to_string())
}

pub(super) fn wav_transcription_form(
    audio: &[u8],
    model: &str,
    prompt: Option<&str>,
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

    Ok(form)
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
