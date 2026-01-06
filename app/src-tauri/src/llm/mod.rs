//! LLM providers for text formatting.
//!
//! This module provides LLM-based text formatting for dictation transcripts.
//! It supports multiple providers (OpenAI, Anthropic, Ollama) and uses
//! configurable prompts to clean up and format transcribed speech.

mod anthropic;
mod defaults;
mod gemini;
mod groq;
mod ollama;
mod openai;
mod prompts;

pub use anthropic::AnthropicLlmProvider;
pub use gemini::GeminiLlmProvider;
pub use groq::GroqLlmProvider;
pub use ollama::OllamaLlmProvider;
pub use openai::OpenAiLlmProvider;
pub use defaults::default_llm_model_for_provider;
pub use prompts::{combine_prompt_sections, PromptSections, SYSTEM_PROMPT_DEFAULT};

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use serde_json::Value as JsonValue;

/// Default timeout for LLM API requests
pub const DEFAULT_LLM_TIMEOUT: Duration = Duration::from_secs(30);

/// Errors that can occur during LLM operations
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API error: {0}")]
    Api(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Request timed out after {0:?}")]
    Timeout(Duration),

    #[error("No API key configured for provider: {0}")]
    NoApiKey(String),

    #[error("Provider not available: {0}")]
    ProviderNotAvailable(String),
}

/// Trait for LLM providers that can format text
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Complete a prompt and return the response
    async fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String, LlmError>;

    /// Complete a prompt and return a structured JSON response.
    ///
    /// Providers that support native JSON schema / structured outputs should override this.
    /// The default implementation falls back to an instruction-based approach and attempts
    /// to parse the response as JSON.
    async fn complete_json_schema(
        &self,
        system_prompt: &str,
        user_message: &str,
        schema_name: &str,
        schema_description: &str,
        schema: JsonValue,
    ) -> Result<JsonValue, LlmError> {
        let system = format!(
            "{}\n\nReturn ONLY valid JSON that matches the provided JSON Schema (no markdown, no extra keys).\nSchema name: {}\nSchema description: {}\nSchema: {}",
            system_prompt,
            schema_name,
            schema_description,
            schema
        );

        let out = self.complete(&system, user_message).await?;
        serde_json::from_str::<JsonValue>(out.trim()).map_err(|e| {
            LlmError::InvalidResponse(format!(
                "Structured output was not valid JSON: {} (content: {})",
                e,
                out
            ))
        })
    }

    /// Get the provider name
    fn name(&self) -> &'static str;

    /// Get the current model being used
    fn model(&self) -> &str;
}

/// Registry of available LLM providers
#[cfg_attr(not(test), allow(dead_code))]
pub struct LlmRegistry {
    providers: Vec<Arc<dyn LlmProvider>>,
    current: String,
}

#[cfg_attr(not(test), allow(dead_code))]
impl LlmRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            current: String::new(),
        }
    }

    /// Register a provider
    pub fn register(&mut self, provider: Arc<dyn LlmProvider>) {
        let name = provider.name().to_string();
        self.providers.push(provider);
        if self.current.is_empty() {
            self.current = name;
        }
    }

    /// Set the current provider by name
    pub fn set_current(&mut self, name: &str) -> bool {
        if self.providers.iter().any(|p| p.name() == name) {
            self.current = name.to_string();
            true
        } else {
            false
        }
    }

    /// Get the current provider
    pub fn current(&self) -> Option<Arc<dyn LlmProvider>> {
        self.providers
            .iter()
            .find(|p| p.name() == self.current)
            .cloned()
    }

    /// Get a provider by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn LlmProvider>> {
        self.providers.iter().find(|p| p.name() == name).cloned()
    }

    /// List all registered provider names
    pub fn list_providers(&self) -> Vec<String> {
        self.providers.iter().map(|p| p.name().to_string()).collect()
    }

    /// Get the current provider name
    pub fn current_name(&self) -> &str {
        &self.current
    }
}

impl Default for LlmRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for LLM formatting
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// Whether LLM formatting is enabled
    pub enabled: bool,
    /// The provider to use (openai, anthropic, ollama)
    pub provider: String,
    /// API key for cloud providers
    pub api_key: String,
    /// Model to use (e.g., "gpt-4o-mini", "claude-3-haiku", "llama3")
    pub model: Option<String>,
    /// Base URL for Ollama (default: http://localhost:11434)
    pub ollama_url: Option<String>,

    /// OpenAI reasoning effort (gpt-5 and o-series models only).
    /// Examples: "low", "medium", "high".
    pub openai_reasoning_effort: Option<String>,

    /// Gemini thinking controls.
    ///
    /// - For Gemini 2.5 models, set `gemini_thinking_budget` (integer token budget).
    /// - For Gemini 3 Pro, set `gemini_thinking_level` ("low" or "high").
    /// - For Gemini 3 Flash, set `gemini_thinking_level` ("minimal", "low", "medium", "high").
    pub gemini_thinking_budget: Option<i64>,
    pub gemini_thinking_level: Option<String>,

    /// Anthropic extended thinking controls.
    ///
    /// When supported by the selected Claude model, setting a budget enables
    /// extended thinking mode.
    pub anthropic_thinking_budget: Option<i64>,
    /// Prompt sections configuration
    pub prompts: PromptSections,
    /// Optional per-program prompt overrides (matched against the foreground executable path)
    pub program_prompt_profiles: Vec<ProgramPromptProfile>,
    /// Request timeout
    pub timeout: Duration,
}

/// A preset/mode within a program profile.
///
/// This is a runtime representation (used by the pipeline). The persisted version lives in
/// `crate::settings`.
#[derive(Debug, Clone)]
pub struct ProgramPreset {
    pub id: String,
    pub name: String,
    pub description: Option<String>,

    /// Example utterances / short hints used by the intent router.
    pub routing_hints: Vec<String>,

    /// Prompt sections to use when this preset is selected.
    pub prompts: PromptSections,

    /// Optional per-preset gate for rewrite (falls back to profile/global settings).
    pub rewrite_llm_enabled: Option<bool>,

    // Optional per-preset overrides for the pipeline
    pub stt_provider: Option<String>,
    pub stt_model: Option<String>,
    pub stt_timeout_seconds: Option<f64>,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,

    // Optional per-preset provider-specific thinking/reasoning knobs.
    pub openai_reasoning_effort: Option<String>,
    pub gemini_thinking_budget: Option<i64>,
    pub gemini_thinking_level: Option<String>,
    pub anthropic_thinking_budget: Option<i64>,
}

/// Per-program prompt override profile.
///
/// If the active/foreground executable path matches any entry in `program_paths`, `prompts` is used instead of
/// the default `LlmConfig.prompts`.
#[derive(Debug, Clone)]
pub struct ProgramPromptProfile {
    #[cfg_attr(not(test), allow(dead_code))]
    pub id: String,
    pub name: String,
    pub program_paths: Vec<String>,
    /// Base prompt sections for this program profile.
    ///
    /// When presets are configured, the selected preset's `prompts` should be used for the
    /// rewrite step instead.
    pub prompts: PromptSections,

    /// Presets/modes within this program profile.
    pub presets: Vec<ProgramPreset>,
    /// Default preset used when routing is off/undecided.
    pub default_preset_id: Option<String>,
    /// Description for the implicit "Default" (no preset) routing target.
    pub default_preset_description: Option<String>,
    /// Persisted manual selection (if set, can be used as an override for routing).
    pub active_preset_id: Option<String>,
    /// Optional intent router configuration.
    pub router: Option<crate::settings::IntentRouterSettings>,

    /// Optional per-profile gate for rewrite (falls back to LlmConfig.enabled)
    pub rewrite_llm_enabled: Option<bool>,

    // Optional per-profile overrides for the pipeline
    pub stt_provider: Option<String>,
    pub stt_model: Option<String>,
    pub stt_timeout_seconds: Option<f64>,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,

    // Optional per-profile provider-specific thinking/reasoning knobs.
    // (These override LlmConfig's global knobs when the profile is active.)
    pub openai_reasoning_effort: Option<String>,
    pub gemini_thinking_budget: Option<i64>,
    pub gemini_thinking_level: Option<String>,
    pub anthropic_thinking_budget: Option<i64>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "openai".to_string(),
            api_key: String::new(),
            model: None,
            ollama_url: None,
            openai_reasoning_effort: None,
            gemini_thinking_budget: None,
            gemini_thinking_level: None,
            anthropic_thinking_budget: None,
            prompts: PromptSections::default(),
            program_prompt_profiles: Vec::new(),
            timeout: DEFAULT_LLM_TIMEOUT,
        }
    }
}

/// Format text using an LLM provider
pub async fn format_text(
    provider: &dyn LlmProvider,
    transcript: &str,
    prompts: &PromptSections,
) -> Result<String, LlmError> {
    if transcript.trim().is_empty() {
        return Ok(String::new());
    }

    let system_prompt = combine_prompt_sections(prompts);
    let result = provider.complete(&system_prompt, transcript).await?;

    Ok(result.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_registry() {
        let registry = LlmRegistry::new();
        assert!(registry.current().is_none());
        assert!(registry.list_providers().is_empty());
    }

    #[test]
    fn test_llm_config_default() {
        let config = LlmConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.provider, "openai");
        assert_eq!(config.timeout, DEFAULT_LLM_TIMEOUT);
    }
}
