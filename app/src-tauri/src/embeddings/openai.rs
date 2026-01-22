use reqwest::Client;
use serde_json::Value as JsonValue;

use super::openai_compat;

const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com";

fn embeddings_url_for_base_url(base_url: &str) -> String {
    crate::http::join_base_url(base_url, "/v1/embeddings")
}

#[derive(Debug, thiserror::Error)]
pub enum OpenAiEmbeddingsError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("OpenAI embeddings request failed: {0}")]
    Api(String),

    #[error("OpenAI embeddings response missing embedding")]
    MissingEmbedding,
}

impl From<openai_compat::OpenAiCompatEmbeddingsError> for OpenAiEmbeddingsError {
    fn from(value: openai_compat::OpenAiCompatEmbeddingsError) -> Self {
        match value {
            openai_compat::OpenAiCompatEmbeddingsError::Http(e) => Self::Http(e),
            openai_compat::OpenAiCompatEmbeddingsError::Api(msg) => Self::Api(msg),
            openai_compat::OpenAiCompatEmbeddingsError::MissingEmbedding => Self::MissingEmbedding,
        }
    }
}

pub async fn embed_text(
    client: &Client,
    api_key: &str,
    model: &str,
    input: &str,
) -> Result<Vec<f32>, OpenAiEmbeddingsError> {
    embed_text_with_base_url(client, api_key, model, input, DEFAULT_OPENAI_BASE_URL).await
}

pub async fn embed_text_with_base_url(
    client: &Client,
    api_key: &str,
    model: &str,
    input: &str,
    base_url: &str,
) -> Result<Vec<f32>, OpenAiEmbeddingsError> {
    let url = embeddings_url_for_base_url(base_url);
    embed_text_with_url(client, api_key, model, input, &url).await
}

pub(crate) async fn embed_text_with_url(
    client: &Client,
    api_key: &str,
    model: &str,
    input: &str,
    url: &str,
) -> Result<Vec<f32>, OpenAiEmbeddingsError> {
    openai_compat::embed_text_with_url(client, api_key, model, input, url)
        .await
        .map_err(Into::into)
}

pub async fn embed_text_with_debug(
    client: &Client,
    api_key: &str,
    model: &str,
    input: &str,
) -> Result<(Vec<f32>, JsonValue, JsonValue), OpenAiEmbeddingsError> {
    let url = embeddings_url_for_base_url(DEFAULT_OPENAI_BASE_URL);
    openai_compat::embed_text_with_debug(client, api_key, model, input, &url)
        .await
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::embeddings_url_for_base_url;

    #[test]
    fn embeddings_url_trims_trailing_slash() {
        assert_eq!(
            embeddings_url_for_base_url("https://api.openai.com"),
            "https://api.openai.com/v1/embeddings"
        );
        assert_eq!(
            embeddings_url_for_base_url("https://api.openai.com/"),
            "https://api.openai.com/v1/embeddings"
        );
    }
}
