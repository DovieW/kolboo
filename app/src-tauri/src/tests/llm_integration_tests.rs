//! Integration tests for LLM providers.
//!
//! These tests verify that LLM providers can be created and configured correctly.
//! Note: Actual API calls require API keys - run with `cargo test -- --ignored`
//! when you have `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, or a running Ollama instance.

use crate::llm::{
    format_text, AnthropicLlmProvider, LlmProvider, OllamaLlmProvider, OpenAiLlmProvider,
    PromptSections,
};

#[test]
fn test_openai_llm_provider_implements_trait() {
    let provider = OpenAiLlmProvider::new("test_key".to_string());
    assert_eq!(provider.name(), "openai");
    assert_eq!(provider.model(), "gpt-4o-mini");
}

#[test]
fn test_anthropic_llm_provider_implements_trait() {
    let provider = AnthropicLlmProvider::new("test_key".to_string());
    assert_eq!(provider.name(), "anthropic");
    assert_eq!(provider.model(), "claude-3-haiku-20240307");
}

#[test]
fn test_ollama_llm_provider_implements_trait() {
    let provider = OllamaLlmProvider::new();
    assert_eq!(provider.name(), "ollama");
    assert_eq!(provider.model(), "llama3.2");
}

#[test]
fn test_openai_llm_provider_with_custom_model() {
    let provider = OpenAiLlmProvider::with_model("test_key".to_string(), "gpt-4o".to_string());
    assert_eq!(provider.name(), "openai");
    assert_eq!(provider.model(), "gpt-4o");
}

#[test]
fn test_anthropic_llm_provider_with_custom_model() {
    let provider = AnthropicLlmProvider::with_model(
        "test_key".to_string(),
        "claude-3-5-sonnet-20241022".to_string(),
    );
    assert_eq!(provider.name(), "anthropic");
    assert_eq!(provider.model(), "claude-3-5-sonnet-20241022");
}

#[test]
fn test_ollama_llm_provider_with_custom_model() {
    let provider = OllamaLlmProvider::with_model("mistral".to_string());
    assert_eq!(provider.name(), "ollama");
    assert_eq!(provider.model(), "mistral");
}

#[test]
fn test_ollama_llm_provider_with_custom_url() {
    let provider =
        OllamaLlmProvider::with_url("http://custom:11434".to_string(), Some("phi3".to_string()));
    assert_eq!(provider.name(), "ollama");
    assert_eq!(provider.model(), "phi3");
}

#[test]
fn test_prompt_sections_default() {
    let prompts = PromptSections::default();
    // Check system_prompt method returns non-empty default
    assert!(!prompts.system_prompt().is_empty());
}

/// Integration test for OpenAI LLM provider.
/// Only runs if OPENAI_API_KEY is set.
#[tokio::test]
#[ignore]
async fn test_openai_llm_complete_integration() {
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            eprintln!("Skipping OpenAI LLM integration test: OPENAI_API_KEY not set");
            return;
        }
    };

    let provider = OpenAiLlmProvider::new(api_key);
    let result = provider.complete("You are a helpful assistant.", "Say hello").await;

    assert!(result.is_ok(), "OpenAI complete failed: {:?}", result);
    let response = result.unwrap();
    assert!(!response.is_empty());
}

/// Integration test for Anthropic LLM provider.
/// Only runs if ANTHROPIC_API_KEY is set.
#[tokio::test]
#[ignore]
async fn test_anthropic_llm_complete_integration() {
    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            eprintln!("Skipping Anthropic LLM integration test: ANTHROPIC_API_KEY not set");
            return;
        }
    };

    let provider = AnthropicLlmProvider::new(api_key);
    let result = provider.complete("You are a helpful assistant.", "Say hello").await;

    assert!(result.is_ok(), "Anthropic complete failed: {:?}", result);
    let response = result.unwrap();
    assert!(!response.is_empty());
}

/// Integration test for Ollama LLM provider.
/// Only runs if Ollama is running locally.
#[tokio::test]
#[ignore]
async fn test_ollama_llm_complete_integration() {
    // Try to connect to Ollama
    let client = reqwest::Client::new();
    let check = client
        .get("http://localhost:11434/api/tags")
        .send()
        .await;

    if check.is_err() {
        eprintln!("Skipping Ollama LLM integration test: Ollama not running");
        return;
    }

    let provider = OllamaLlmProvider::new();
    let result = provider.complete("You are a helpful assistant.", "Say hello").await;

    assert!(result.is_ok(), "Ollama complete failed: {:?}", result);
    let response = result.unwrap();
    assert!(!response.is_empty());
}

/// Integration test for format_text function.
/// Only runs if OPENAI_API_KEY is set.
#[tokio::test]
#[ignore]
async fn test_format_text_integration() {
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            eprintln!("Skipping format_text integration test: OPENAI_API_KEY not set");
            return;
        }
    };

    let provider = OpenAiLlmProvider::new(api_key);
    let prompts = PromptSections::default();

    let result = format_text(&provider, "um hello there uh how are you", &prompts).await;

    assert!(result.is_ok(), "format_text failed: {:?}", result);
    let formatted = result.unwrap();
    // The LLM should clean up filler words
    assert!(!formatted.is_empty());
}

/// Test that format_text returns empty string for empty input.
#[tokio::test]
async fn test_format_text_empty_input() {
    let provider = OpenAiLlmProvider::new("test_key".to_string());
    let prompts = PromptSections::default();

    let result = format_text(&provider, "", &prompts).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

/// Test that format_text returns empty string for whitespace-only input.
#[tokio::test]
async fn test_format_text_whitespace_input() {
    let provider = OpenAiLlmProvider::new("test_key".to_string());
    let prompts = PromptSections::default();

    let result = format_text(&provider, "   \n\t   ", &prompts).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}
