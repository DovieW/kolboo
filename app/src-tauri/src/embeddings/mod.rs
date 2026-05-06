pub mod cohere;
pub mod fireworks;
pub mod openai;

mod debug;
mod openai_compat;

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::sync::Arc;

/// Error type for embeddings operations
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingsError {
    // Reserved for adapters that can distinguish transport failure from a
    // provider-level API error. Keep the variant so callers do not need a
    // breaking error-shape change when we add that adapter.
    #[allow(dead_code)]
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("API error: {0}")]
    Api(String),

    // Provider helpers currently normalize missing embeddings into `Api(...)`.
    // Keep the precise variant for future adapters that can surface it directly.
    #[allow(dead_code)]
    #[error("Missing embedding in response")]
    MissingEmbedding,

    #[error("No API key configured for provider: {0}")]
    NoApiKey(String),

    #[error("Provider not available: {0}")]
    ProviderNotAvailable(String),
}

/// Trait for embeddings providers that can embed text into vector representations.
///
/// This trait is designed to be injectable for testing, allowing deterministic
/// offline tests without making real API calls.
#[async_trait]
pub trait EmbeddingsProvider: Send + Sync {
    /// Embed a single text into a vector representation.
    ///
    /// Returns the embedding vector along with request/response JSON for debugging.
    async fn embed_text(
        &self,
        text: &str,
        input_type: Option<&str>,
    ) -> Result<(Vec<f32>, JsonValue, JsonValue), EmbeddingsError>;

    /// Get the provider name (e.g., "openai", "cohere", "fireworks")
    #[allow(dead_code)]
    fn name(&self) -> &'static str;

    /// Get the current model being used
    #[allow(dead_code)]
    fn model(&self) -> &str;
}

/// A boxed embeddings provider for dynamic dispatch
pub type BoxedEmbeddingsProvider = Arc<dyn EmbeddingsProvider>;

/// Provider input role. Some embeddings APIs need different input-type hints
/// for a user query vs. a document/candidate hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingInputRole {
    Query,
    Document,
}

pub fn is_supported_provider(provider_id: &str) -> bool {
    matches!(provider_id, "openai" | "cohere" | "fireworks")
}

pub fn default_model_for_provider(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        "cohere" => Some("embed-english-v3.0"),
        // Starter default: keep in sync with the UI model list.
        "fireworks" => Some("fireworks/qwen3-embedding-0p6b"),
        "openai" => Some("text-embedding-3-small"),
        _ => None,
    }
}

pub fn input_type_for_provider(
    provider_id: &str,
    role: EmbeddingInputRole,
) -> Option<&'static str> {
    match (provider_id, role) {
        ("cohere", EmbeddingInputRole::Query) => Some("search_query"),
        ("cohere", EmbeddingInputRole::Document) => Some("search_document"),
        _ => None,
    }
}

pub fn build_provider(
    client: reqwest::Client,
    provider_id: &str,
    api_key: String,
    model: String,
) -> Result<BoxedEmbeddingsProvider, EmbeddingsError> {
    if api_key.trim().is_empty() {
        return Err(EmbeddingsError::NoApiKey(provider_id.to_string()));
    }

    let provider: BoxedEmbeddingsProvider = match provider_id {
        "openai" => Arc::new(OpenAiEmbeddingsProvider::new(client, api_key, model)),
        "cohere" => Arc::new(CohereEmbeddingsProvider::new(client, api_key, model)),
        "fireworks" => Arc::new(FireworksEmbeddingsProvider::new(client, api_key, model)),
        other => return Err(EmbeddingsError::ProviderNotAvailable(other.to_string())),
    };

    Ok(provider)
}

// --------------------------------------------------------------------------
// Concrete provider implementations wrapping existing API functions
// --------------------------------------------------------------------------

/// OpenAI embeddings provider
pub struct OpenAiEmbeddingsProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl OpenAiEmbeddingsProvider {
    pub fn new(client: reqwest::Client, api_key: String, model: String) -> Self {
        Self {
            client,
            api_key,
            model,
        }
    }
}

#[async_trait]
impl EmbeddingsProvider for OpenAiEmbeddingsProvider {
    async fn embed_text(
        &self,
        text: &str,
        _input_type: Option<&str>,
    ) -> Result<(Vec<f32>, JsonValue, JsonValue), EmbeddingsError> {
        openai::embed_text_with_debug(&self.client, &self.api_key, &self.model, text)
            .await
            .map_err(|e| EmbeddingsError::Api(e.to_string()))
    }

    fn name(&self) -> &'static str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

/// Cohere embeddings provider
pub struct CohereEmbeddingsProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl CohereEmbeddingsProvider {
    pub fn new(client: reqwest::Client, api_key: String, model: String) -> Self {
        Self {
            client,
            api_key,
            model,
        }
    }
}

#[async_trait]
impl EmbeddingsProvider for CohereEmbeddingsProvider {
    async fn embed_text(
        &self,
        text: &str,
        input_type: Option<&str>,
    ) -> Result<(Vec<f32>, JsonValue, JsonValue), EmbeddingsError> {
        // Cohere requires an input_type; default to "search_query" if not specified
        let input_type = input_type.unwrap_or("search_query");
        cohere::embed_text_with_debug(&self.client, &self.api_key, &self.model, input_type, text)
            .await
            .map_err(|e| EmbeddingsError::Api(e.to_string()))
    }

    fn name(&self) -> &'static str {
        "cohere"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

/// Fireworks embeddings provider
pub struct FireworksEmbeddingsProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl FireworksEmbeddingsProvider {
    pub fn new(client: reqwest::Client, api_key: String, model: String) -> Self {
        Self {
            client,
            api_key,
            model,
        }
    }
}

#[async_trait]
impl EmbeddingsProvider for FireworksEmbeddingsProvider {
    async fn embed_text(
        &self,
        text: &str,
        _input_type: Option<&str>,
    ) -> Result<(Vec<f32>, JsonValue, JsonValue), EmbeddingsError> {
        fireworks::embed_text_with_debug(&self.client, &self.api_key, &self.model, text)
            .await
            .map_err(|e| EmbeddingsError::Api(e.to_string()))
    }

    fn name(&self) -> &'static str {
        "fireworks"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> Option<f32> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    if norm_a <= 0.0 || norm_b <= 0.0 {
        return None;
    }

    Some(dot / (norm_a.sqrt() * norm_b.sqrt()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_identical_vectors() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v).unwrap();
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "identical vectors should have similarity 1.0"
        );
    }

    #[test]
    fn cosine_similarity_opposite_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!(
            (sim - (-1.0)).abs() < 1e-6,
            "opposite vectors should have similarity -1.0"
        );
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!(
            sim.abs() < 1e-6,
            "orthogonal vectors should have similarity 0.0"
        );
    }

    #[test]
    fn cosine_similarity_mismatched_lengths() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0];
        assert!(cosine_similarity(&a, &b).is_none());
    }

    #[test]
    fn cosine_similarity_empty_vectors() {
        let empty: Vec<f32> = vec![];
        assert!(cosine_similarity(&empty, &empty).is_none());
    }

    #[test]
    fn cosine_similarity_zero_vector() {
        let zero = vec![0.0, 0.0, 0.0];
        let non_zero = vec![1.0, 2.0, 3.0];
        assert!(cosine_similarity(&zero, &non_zero).is_none());
    }

    #[test]
    fn cosine_similarity_similar_direction() {
        // Two vectors pointing in roughly the same direction.
        let a = vec![1.0, 1.0];
        let b = vec![2.0, 2.0];
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "parallel vectors should have similarity 1.0 regardless of magnitude"
        );
    }

    #[test]
    fn cosine_similarity_realistic_embeddings() {
        // Simulated embedding vectors (3 dimensions for simplicity).
        // "hello" and "hi" should be similar; "hello" and "goodbye" less so.
        let hello = vec![0.9, 0.1, 0.0];
        let hi = vec![0.85, 0.15, 0.05];
        let goodbye = vec![0.1, 0.9, 0.0];

        let sim_hello_hi = cosine_similarity(&hello, &hi).unwrap();
        let sim_hello_goodbye = cosine_similarity(&hello, &goodbye).unwrap();

        assert!(
            sim_hello_hi > sim_hello_goodbye,
            "similar words should have higher similarity"
        );
        assert!(sim_hello_hi > 0.9, "similar words should be highly similar");
    }

    #[test]
    fn provider_defaults_and_input_roles_are_provider_specific() {
        assert!(is_supported_provider("openai"));
        assert!(is_supported_provider("cohere"));
        assert!(is_supported_provider("fireworks"));
        assert!(!is_supported_provider("unknown"));

        assert_eq!(
            default_model_for_provider("openai"),
            Some("text-embedding-3-small")
        );
        assert_eq!(
            input_type_for_provider("cohere", EmbeddingInputRole::Query),
            Some("search_query")
        );
        assert_eq!(
            input_type_for_provider("cohere", EmbeddingInputRole::Document),
            Some("search_document")
        );
        assert_eq!(
            input_type_for_provider("openai", EmbeddingInputRole::Document),
            None
        );
    }

    #[test]
    fn build_provider_constructs_multiple_concrete_adapters() {
        let openai = build_provider(
            reqwest::Client::new(),
            "openai",
            "test-key".to_string(),
            "text-embedding-3-small".to_string(),
        )
        .expect("openai embeddings provider");
        let cohere = build_provider(
            reqwest::Client::new(),
            "cohere",
            "test-key".to_string(),
            "embed-english-v3.0".to_string(),
        )
        .expect("cohere embeddings provider");

        assert_eq!(openai.name(), "openai");
        assert_eq!(cohere.name(), "cohere");
    }
}
