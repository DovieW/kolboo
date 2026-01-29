use crate::ocr::OcrResult;
use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f64,
    top_p: f64,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: Vec<ChatContentPart>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ChatContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Serialize)]
struct ImageUrl {
    url: String,
}

/// Parameters for an OCR request (reduces function argument count).
pub struct OcrRequestParams<'a> {
    pub base_url: &'a str,
    pub model: &'a str,
    pub image_png: &'a [u8],
    pub api_key: Option<&'a str>,
    pub timeout_ms: u64,
    pub prompt: &'a str,
    pub max_tokens: u32,
    pub temperature: f64,
    pub top_p: f64,
}

fn extract_first_choice_text(response_json: &serde_json::Value) -> Option<String> {
    let choices = response_json.get("choices")?.as_array()?;
    let first = choices.first()?;
    if let Some(text) = first
        .get("message")
        .and_then(|msg| msg.get("content"))
        .and_then(|content| content.as_str())
    {
        return Some(text.to_string());
    }

    if let Some(parts) = first
        .get("message")
        .and_then(|msg| msg.get("content"))
        .and_then(|content| content.as_array())
    {
        let mut out = String::new();
        for part in parts {
            if let Some(text) = part
                .get("text")
                .and_then(|t| t.as_str())
                .or_else(|| part.get("content").and_then(|t| t.as_str()))
            {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(text);
            }
        }
        if !out.trim().is_empty() {
            return Some(out);
        }
    }

    first
        .get("text")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
}

#[allow(dead_code)]
pub async fn request_ocr_text(
    params: OcrRequestParams<'_>,
) -> Result<(OcrResult, serde_json::Value), String> {
    let data_url = format!(
        "data:image/png;base64,{}",
        general_purpose::STANDARD.encode(params.image_png)
    );

    // Build content parts: include text prompt only if non-empty.
    let mut content_parts = Vec::new();
    let prompt_trimmed = params.prompt.trim();
    if !prompt_trimmed.is_empty() {
        content_parts.push(ChatContentPart::Text {
            text: prompt_trimmed.to_string(),
        });
    }
    content_parts.push(ChatContentPart::ImageUrl {
        image_url: ImageUrl { url: data_url },
    });

    let request = ChatCompletionRequest {
        model: params.model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: content_parts,
        }],
        max_tokens: params.max_tokens,
        temperature: params.temperature,
        top_p: params.top_p,
    };

    let client = Client::new();
    let url = crate::http::join_base_url(params.base_url, "/v1/chat/completions");
    let mut req = client.post(url).json(&request);

    if let Some(key) = params.api_key.map(str::trim).filter(|k| !k.is_empty()) {
        req = req.bearer_auth(key);
    }

    let timeout = if params.timeout_ms > 0 {
        Some(Duration::from_millis(params.timeout_ms))
    } else {
        None
    };
    if let Some(timeout) = timeout {
        req = req.timeout(timeout);
    }

    let response = req
        .send()
        .await
        .map_err(|e| format!("OCR request failed: {}", e))?;
    let (status, body) = crate::http::status_and_text(response).await;

    if !status.is_success() {
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| {
                v.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or(body);
        return Err(format!("OCR API error ({}): {}", status, message));
    }

    let response_json = crate::http::parse_json_value(&body)?;
    let text = extract_first_choice_text(&response_json)
        .ok_or_else(|| "OCR response missing content".to_string())?;

    Ok((
        OcrResult {
            text,
            provider: "openai_compatible".to_string(),
            model: params.model.to_string(),
        },
        response_json,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn request_ocr_text_parses_response() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(body_string_contains("You are an OCR engine"))
            .and(body_string_contains("data:image/png;base64"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {"content": "hello world"}
                }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let (result, _response_json) = request_ocr_text(OcrRequestParams {
            base_url: server.uri().as_str(),
            model: "lightonai/LightOnOCR-1B-1025",
            image_png: b"fake_png",
            api_key: None,
            timeout_ms: 2000,
            prompt: "You are an OCR engine. Extract text.",
            max_tokens: 512,
            temperature: 0.0,
            top_p: 1.0,
        })
        .await
        .expect("ocr request should succeed");

        assert_eq!(result.text, "hello world");
        assert_eq!(result.provider, "openai_compatible");
    }

    #[tokio::test]
    async fn request_ocr_text_returns_api_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "error": {"message": "bad request"}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let err = request_ocr_text(OcrRequestParams {
            base_url: server.uri().as_str(),
            model: "model",
            image_png: b"fake_png",
            api_key: Some("key"),
            timeout_ms: 2000,
            prompt: "You are an OCR engine. Extract text.",
            max_tokens: 512,
            temperature: 0.0,
            top_p: 1.0,
        })
        .await
        .expect_err("expected error");

        assert!(err.contains("400"));
        assert!(err.contains("bad request"));
    }
}
