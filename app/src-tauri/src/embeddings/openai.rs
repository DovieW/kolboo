use reqwest::Client;
use serde::Deserialize;
use serde_json::Value as JsonValue;

const OPENAI_EMBEDDINGS_URL: &str = "https://api.openai.com/v1/embeddings";

#[derive(Debug, thiserror::Error)]
pub enum OpenAiEmbeddingsError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("OpenAI embeddings request failed: {0}")]
    Api(String),

    #[error("OpenAI embeddings response missing embedding")]
    MissingEmbedding,
}

#[derive(Debug, Deserialize)]
struct EmbeddingsResponse {
    data: Vec<EmbeddingsData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingsData {
    embedding: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct OpenAiErrorResponse {
    error: OpenAiErrorBody,
}

#[derive(Debug, Deserialize)]
struct OpenAiErrorBody {
    message: String,
}

pub async fn embed_text(
    client: &Client,
    api_key: &str,
    model: &str,
    input: &str,
) -> Result<Vec<f32>, OpenAiEmbeddingsError> {
    embed_text_with_url(client, api_key, model, input, OPENAI_EMBEDDINGS_URL).await
}

pub(crate) async fn embed_text_with_url(
    client: &Client,
    api_key: &str,
    model: &str,
    input: &str,
    url: &str,
) -> Result<Vec<f32>, OpenAiEmbeddingsError> {
    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": model,
            "input": input,
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if let Ok(parsed) = serde_json::from_str::<OpenAiErrorResponse>(&body) {
            return Err(OpenAiEmbeddingsError::Api(format!(
                "{}: {}",
                status, parsed.error.message
            )));
        }
        return Err(OpenAiEmbeddingsError::Api(format!("{}: {}", status, body)));
    }

    let parsed: EmbeddingsResponse = resp.json().await?;
    let embedding = parsed
        .data
        .into_iter()
        .next()
        .ok_or(OpenAiEmbeddingsError::MissingEmbedding)?
        .embedding;

    if embedding.is_empty() {
        return Err(OpenAiEmbeddingsError::MissingEmbedding);
    }

    Ok(embedding)
}

pub async fn embed_text_with_debug(
    client: &Client,
    api_key: &str,
    model: &str,
    input: &str,
) -> Result<(Vec<f32>, JsonValue, JsonValue), OpenAiEmbeddingsError> {
    const INPUT_PREVIEW_MAX_CHARS: usize = 800;

    let input_len = input.chars().count();
    let mut preview: String = input.chars().take(INPUT_PREVIEW_MAX_CHARS).collect();
    let truncated = input_len > INPUT_PREVIEW_MAX_CHARS;
    if truncated {
        preview.push('…');
    }

    let request_json = serde_json::json!({
        "url": OPENAI_EMBEDDINGS_URL,
        "model": model,
        "input_preview": preview,
        "input_len": input_len,
        "input_truncated": truncated,
    });

    let resp = client
        .post(OPENAI_EMBEDDINGS_URL)
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": model,
            "input": input,
        }))
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        if let Ok(parsed) = serde_json::from_str::<OpenAiErrorResponse>(&body) {
            let response_json = serde_json::json!({
                "status": status.as_u16(),
                "error": parsed.error.message,
            });
            return Err(OpenAiEmbeddingsError::Api(format!("{}", response_json)));
        }

        let response_json = serde_json::json!({
            "status": status.as_u16(),
            "body": body,
        });
        return Err(OpenAiEmbeddingsError::Api(format!("{}", response_json)));
    }

    let parsed: JsonValue = serde_json::from_str(&body)
        .map_err(|e| OpenAiEmbeddingsError::Api(format!("Failed to parse response JSON: {}", e)))?;

    // Extract embedding values.
    let embedding_arr = parsed
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|d| d.first())
        .and_then(|x| x.get("embedding"))
        .and_then(|e| e.as_array())
        .ok_or(OpenAiEmbeddingsError::MissingEmbedding)?;

    let mut embedding: Vec<f32> = Vec::with_capacity(embedding_arr.len());
    for v in embedding_arr {
        let n = v.as_f64().ok_or(OpenAiEmbeddingsError::MissingEmbedding)?;
        embedding.push(n as f32);
    }
    if embedding.is_empty() {
        return Err(OpenAiEmbeddingsError::MissingEmbedding);
    }

    // Build a redacted response payload (exclude raw embedding floats).
    let embedding_len = embedding_arr.len();
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
