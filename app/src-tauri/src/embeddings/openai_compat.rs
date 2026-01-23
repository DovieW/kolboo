use reqwest::Client;
use serde::Deserialize;
use serde_json::Value as JsonValue;

use super::debug::input_preview;

#[derive(Debug, thiserror::Error)]
pub enum OpenAiCompatEmbeddingsError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Embeddings request failed: {0}")]
    Api(String),

    #[error("Embeddings response missing embedding")]
    MissingEmbedding,
}

#[derive(Debug, Deserialize)]
struct OpenAiCompatErrorResponse {
    error: OpenAiCompatErrorBody,
}

#[derive(Debug, Deserialize)]
struct OpenAiCompatErrorBody {
    message: String,
}

fn parse_openai_compat_error_message(body: &str) -> Option<String> {
    serde_json::from_str::<OpenAiCompatErrorResponse>(body)
        .ok()
        .map(|p| p.error.message)
}

async fn send_openai_compat_embeddings_request(
    client: &Client,
    api_key: &str,
    model: &str,
    input: &str,
    url: &str,
) -> Result<(reqwest::StatusCode, String), OpenAiCompatEmbeddingsError> {
    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": model,
            "input": input,
        }))
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    Ok((status, body))
}

fn extract_openai_compat_embedding(
    parsed: &JsonValue,
) -> Result<Vec<f32>, OpenAiCompatEmbeddingsError> {
    let embedding_arr = parsed
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|d| d.first())
        .and_then(|x| x.get("embedding"))
        .and_then(|e| e.as_array())
        .ok_or(OpenAiCompatEmbeddingsError::MissingEmbedding)?;

    let mut embedding: Vec<f32> = Vec::with_capacity(embedding_arr.len());
    for v in embedding_arr {
        let n = v
            .as_f64()
            .ok_or(OpenAiCompatEmbeddingsError::MissingEmbedding)?;
        embedding.push(n as f32);
    }

    if embedding.is_empty() {
        return Err(OpenAiCompatEmbeddingsError::MissingEmbedding);
    }

    Ok(embedding)
}

pub async fn embed_text_with_url(
    client: &Client,
    api_key: &str,
    model: &str,
    input: &str,
    url: &str,
) -> Result<Vec<f32>, OpenAiCompatEmbeddingsError> {
    let (status, body) =
        send_openai_compat_embeddings_request(client, api_key, model, input, url).await?;

    if !status.is_success() {
        if let Some(message) = parse_openai_compat_error_message(&body) {
            return Err(OpenAiCompatEmbeddingsError::Api(format!(
                "{}: {}",
                status, message
            )));
        }
        return Err(OpenAiCompatEmbeddingsError::Api(format!(
            "{}: {}",
            status, body
        )));
    }

    let parsed: JsonValue = serde_json::from_str(&body).map_err(|e| {
        OpenAiCompatEmbeddingsError::Api(format!("Failed to parse response JSON: {}", e))
    })?;

    extract_openai_compat_embedding(&parsed)
}

pub async fn embed_text_with_debug(
    client: &Client,
    api_key: &str,
    model: &str,
    input: &str,
    url: &str,
) -> Result<(Vec<f32>, JsonValue, JsonValue), OpenAiCompatEmbeddingsError> {
    const INPUT_PREVIEW_MAX_CHARS: usize = 800;

    let (input_len, preview, truncated) = input_preview(input, INPUT_PREVIEW_MAX_CHARS);

    let request_json = serde_json::json!({
        "url": url,
        "model": model,
        "input_preview": preview,
        "input_len": input_len,
        "input_truncated": truncated,
    });

    let (status, body) =
        send_openai_compat_embeddings_request(client, api_key, model, input, url).await?;

    if !status.is_success() {
        if let Some(message) = parse_openai_compat_error_message(&body) {
            let response_json = serde_json::json!({
                "status": status.as_u16(),
                "error": message,
            });
            return Err(OpenAiCompatEmbeddingsError::Api(format!(
                "{}",
                response_json
            )));
        }

        let response_json = serde_json::json!({
            "status": status.as_u16(),
            "body": body,
        });
        return Err(OpenAiCompatEmbeddingsError::Api(format!(
            "{}",
            response_json
        )));
    }

    let parsed: JsonValue = serde_json::from_str(&body).map_err(|e| {
        OpenAiCompatEmbeddingsError::Api(format!("Failed to parse response JSON: {}", e))
    })?;

    let embedding = extract_openai_compat_embedding(&parsed)?;

    // Build a redacted response payload (exclude raw embedding floats).
    let embedding_len = embedding.len();
    let response_json = serde_json::json!({
        "status": status.as_u16(),
        "model": parsed.get("model").cloned().unwrap_or(JsonValue::Null),
        "usage": parsed.get("usage").cloned().unwrap_or(JsonValue::Null),
        "data": [{
            "embedding_len": embedding_len,
        }],
    });

    Ok((embedding, request_json, response_json))
}

#[cfg(test)]
mod tests {
    use super::parse_openai_compat_error_message;

    #[test]
    fn error_message_parses_openai_shape() {
        let body = r#"{"error":{"message":"nope"}}"#;
        assert_eq!(
            parse_openai_compat_error_message(body),
            Some("nope".to_string())
        );
    }
}
