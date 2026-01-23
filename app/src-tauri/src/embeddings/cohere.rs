use reqwest::Client;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::time::Duration;
use tokio::time::sleep;

const DEFAULT_COHERE_BASE_URL: &str = "https://api.cohere.com";

fn embeddings_url_for_base_url(base_url: &str) -> String {
    crate::http::join_base_url(base_url, "/v2/embed")
}

const COHERE_MAX_RETRIES: usize = 5;
const COHERE_RETRY_BASE_DELAY_MS: u64 = 750;

#[derive(Debug, thiserror::Error)]
pub enum CohereEmbeddingsError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Cohere embeddings request failed: {0}")]
    Api(String),

    #[error("Cohere embeddings response missing embedding")]
    MissingEmbedding,
}

#[derive(Debug, Deserialize)]
struct CohereErrorResponse {
    message: Option<String>,
    error: Option<String>,
}

// Convenience wrapper for single-input embedding. Not all builds use this directly.
#[allow(dead_code)]
pub async fn embed_text(
    client: &Client,
    api_key: &str,
    model: &str,
    input_type: &str,
    input: &str,
) -> Result<Vec<f32>, CohereEmbeddingsError> {
    let inputs: Vec<String> = vec![input.to_string()];
    let mut out = embed_texts(client, api_key, model, input_type, &inputs).await?;
    if out.is_empty() {
        return Err(CohereEmbeddingsError::MissingEmbedding);
    }
    Ok(out.remove(0))
}

pub async fn embed_texts(
    client: &Client,
    api_key: &str,
    model: &str,
    input_type: &str,
    inputs: &[String],
) -> Result<Vec<Vec<f32>>, CohereEmbeddingsError> {
    embed_texts_with_base_url(
        client,
        api_key,
        model,
        input_type,
        inputs,
        DEFAULT_COHERE_BASE_URL,
    )
    .await
}

pub async fn embed_texts_with_base_url(
    client: &Client,
    api_key: &str,
    model: &str,
    input_type: &str,
    inputs: &[String],
    base_url: &str,
) -> Result<Vec<Vec<f32>>, CohereEmbeddingsError> {
    let url = embeddings_url_for_base_url(base_url);
    embed_texts_with_url(client, api_key, model, input_type, inputs, &url).await
}

#[allow(dead_code)]
pub async fn embed_text_with_base_url(
    client: &Client,
    api_key: &str,
    model: &str,
    input_type: &str,
    input: &str,
    base_url: &str,
) -> Result<Vec<f32>, CohereEmbeddingsError> {
    let inputs: Vec<String> = vec![input.to_string()];
    let mut out =
        embed_texts_with_base_url(client, api_key, model, input_type, &inputs, base_url).await?;
    if out.is_empty() {
        return Err(CohereEmbeddingsError::MissingEmbedding);
    }
    Ok(out.remove(0))
}

pub(crate) async fn embed_texts_with_url(
    client: &Client,
    api_key: &str,
    model: &str,
    input_type: &str,
    inputs: &[String],
    url: &str,
) -> Result<Vec<Vec<f32>>, CohereEmbeddingsError> {
    if inputs.is_empty() {
        return Ok(vec![]);
    }

    let mut last_err: Option<CohereEmbeddingsError> = None;

    for attempt in 0..=COHERE_MAX_RETRIES {
        let resp = client
            .post(url)
            .bearer_auth(api_key)
            .json(&serde_json::json!({
                "model": model,
                "texts": inputs,
                "input_type": input_type,
                // Force predictable numeric format.
                "embedding_types": ["float"],
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();

            // Rate limit / transient errors: backoff + retry.
            let is_retryable = status.as_u16() == 429 || status.is_server_error();
            let retry_after_seconds = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());

            let body = resp.text().await.unwrap_or_default();
            let msg = if let Ok(parsed) = serde_json::from_str::<CohereErrorResponse>(&body) {
                parsed
                    .message
                    .or(parsed.error)
                    .unwrap_or_else(|| body.clone())
            } else {
                body.clone()
            };

            let err = CohereEmbeddingsError::Api(format!("{}: {}", status, msg));
            last_err = Some(err);

            if is_retryable && attempt < COHERE_MAX_RETRIES {
                // Exponential backoff, but respect Retry-After if provided.
                let exp_ms = COHERE_RETRY_BASE_DELAY_MS.saturating_mul(1u64 << attempt);
                let retry_after_ms = retry_after_seconds.unwrap_or(0).saturating_mul(1000);
                let wait_ms = std::cmp::max(exp_ms, retry_after_ms);
                sleep(Duration::from_millis(wait_ms)).await;
                continue;
            }

            return Err(last_err.unwrap());
        }

        let parsed: JsonValue = resp.json().await?;

        // Cohere v2 embed returns either:
        // - embeddings: { float: [[...], ...] }
        // - embeddings: [[...], ...] (older)
        let embeddings_root = parsed
            .get("embeddings")
            .ok_or(CohereEmbeddingsError::MissingEmbedding)?;
        let embeddings_arr = if embeddings_root.is_object() {
            embeddings_root
                .get("float")
                .and_then(|e| e.as_array())
                .ok_or(CohereEmbeddingsError::MissingEmbedding)?
        } else {
            embeddings_root
                .as_array()
                .ok_or(CohereEmbeddingsError::MissingEmbedding)?
        };

        if embeddings_arr.len() != inputs.len() {
            return Err(CohereEmbeddingsError::Api(format!(
                "Unexpected embeddings count (got {}, expected {})",
                embeddings_arr.len(),
                inputs.len()
            )));
        }

        let mut out: Vec<Vec<f32>> = Vec::with_capacity(embeddings_arr.len());
        for row in embeddings_arr {
            let row_arr = row
                .as_array()
                .ok_or(CohereEmbeddingsError::MissingEmbedding)?;
            let mut embedding: Vec<f32> = Vec::with_capacity(row_arr.len());
            for v in row_arr {
                let n = v.as_f64().ok_or(CohereEmbeddingsError::MissingEmbedding)?;
                embedding.push(n as f32);
            }
            if embedding.is_empty() {
                return Err(CohereEmbeddingsError::MissingEmbedding);
            }
            out.push(embedding);
        }

        return Ok(out);
    }

    Err(last_err.unwrap_or_else(|| CohereEmbeddingsError::Api("Unknown error".to_string())))
}

pub async fn embed_text_with_debug(
    client: &Client,
    api_key: &str,
    model: &str,
    input_type: &str,
    input: &str,
) -> Result<(Vec<f32>, JsonValue, JsonValue), CohereEmbeddingsError> {
    const INPUT_PREVIEW_MAX_CHARS: usize = 800;

    let (input_len, preview, truncated) =
        super::debug::input_preview(input, INPUT_PREVIEW_MAX_CHARS);

    let url = embeddings_url_for_base_url(DEFAULT_COHERE_BASE_URL);
    let url_for_request = url.clone();

    let request_json = serde_json::json!({
        "url": url,
        "model": model,
        "input_type": input_type,
        "input_preview": preview,
        "input_len": input_len,
        "input_truncated": truncated,
    });

    let resp = client
        .post(url_for_request)
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": model,
            "texts": [input],
            "input_type": input_type,
            "embedding_types": ["float"],
        }))
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        if let Ok(parsed) = serde_json::from_str::<CohereErrorResponse>(&body) {
            let msg = parsed
                .message
                .or(parsed.error)
                .unwrap_or_else(|| body.clone());
            let response_json = serde_json::json!({
                "status": status.as_u16(),
                "error": msg,
            });
            return Err(CohereEmbeddingsError::Api(format!("{}", response_json)));
        }

        let response_json = serde_json::json!({
            "status": status.as_u16(),
            "body": body,
        });
        return Err(CohereEmbeddingsError::Api(format!("{}", response_json)));
    }

    let parsed: JsonValue =
        crate::http::parse_json_value(&body).map_err(CohereEmbeddingsError::Api)?;

    let embedding_arr_opt = parsed
        .get("embeddings")
        .and_then(|e| {
            if e.is_object() {
                e.get("float")
            } else {
                Some(e)
            }
        })
        .and_then(|e| e.as_array())
        .and_then(|arr| arr.first())
        .and_then(|first| first.as_array());

    let embedding_arr = embedding_arr_opt.ok_or(CohereEmbeddingsError::MissingEmbedding)?;

    let mut embedding: Vec<f32> = Vec::with_capacity(embedding_arr.len());
    for v in embedding_arr {
        let n = v.as_f64().ok_or(CohereEmbeddingsError::MissingEmbedding)?;
        embedding.push(n as f32);
    }

    if embedding.is_empty() {
        return Err(CohereEmbeddingsError::MissingEmbedding);
    }

    let embedding_len = embedding_arr.len();

    // Build a redacted response payload (exclude raw embedding floats).
    let response_json = serde_json::json!({
        "status": status.as_u16(),
        "model": parsed.get("model").cloned().unwrap_or(JsonValue::Null),
        "meta": parsed.get("meta").cloned().unwrap_or(JsonValue::Null),
        "embeddings": [{
            "embedding_len": embedding_len,
        }],
    });

    Ok((embedding, request_json, response_json))
}

#[cfg(test)]
mod tests {
    use super::embeddings_url_for_base_url;

    #[test]
    fn embeddings_url_trims_trailing_slash() {
        assert_eq!(
            embeddings_url_for_base_url("https://api.cohere.com"),
            "https://api.cohere.com/v2/embed"
        );
        assert_eq!(
            embeddings_url_for_base_url("https://api.cohere.com/"),
            "https://api.cohere.com/v2/embed"
        );
    }
}
