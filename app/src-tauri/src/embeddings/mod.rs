// Allow dead code for provider infrastructure that's not yet wired into production
// but is used by tests and will be used when embeddings routing is fully integrated.
#![allow(dead_code)]

pub mod cohere;
pub mod fireworks;
pub mod openai;

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::sync::Arc;

/// Error type for embeddings operations
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingsError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("API error: {0}")]
    Api(String),

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
    fn name(&self) -> &'static str;

    /// Get the current model being used
    fn model(&self) -> &str;
}

/// A boxed embeddings provider for dynamic dispatch
pub type BoxedEmbeddingsProvider = Arc<dyn EmbeddingsProvider>;

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
}
