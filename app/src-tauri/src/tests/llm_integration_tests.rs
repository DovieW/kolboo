//! Integration tests for LLM providers.
//!
//! These tests verify that LLM providers can be created and configured correctly.
//! Note: Actual API calls require API keys - run with `cargo test -- --ignored`
//! when you have `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, or a running Ollama instance.

use crate::llm::{
    format_text, AnthropicLlmProvider, CohereLlmProvider, FireworksLlmProvider, GroqLlmProvider,
    LlmError, LlmProvider, OllamaLlmProvider, OpenAiLlmProvider, PromptSections,
};

use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
    let result = provider
        .complete("You are a helpful assistant.", "Say hello")
        .await;

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
    let result = provider
        .complete("You are a helpful assistant.", "Say hello")
        .await;

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
    let check = client.get("http://localhost:11434/api/tags").send().await;

    if check.is_err() {
        eprintln!("Skipping Ollama LLM integration test: Ollama not running");
        return;
    }

    let provider = OllamaLlmProvider::new();
    let result = provider
        .complete("You are a helpful assistant.", "Say hello")
        .await;

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

#[tokio::test]
async fn test_ollama_complete_sends_expected_request_body() {
    let mock_server = MockServer::start().await;

    let expected_request = json!({
        "model": "test-model",
        "messages": [
            {"role": "system", "content": "sys"},
            {"role": "user", "content": "user"}
        ],
        "stream": false,
        "options": {
            "temperature": 0.3,
            "num_predict": 4096
        }
    });

    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .and(body_json(&expected_request))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": { "content": "hello" }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = OllamaLlmProvider::with_url(mock_server.uri(), Some("test-model".to_string()));
    let out = provider.complete("sys", "user").await.unwrap();
    assert_eq!(out, "hello");
}

#[tokio::test]
async fn test_ollama_complete_parses_json_error_body() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "model not found"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = OllamaLlmProvider::with_url(mock_server.uri(), Some("test-model".to_string()));
    let err = provider
        .complete("sys", "user")
        .await
        .expect_err("expected error");

    match err {
        LlmError::Api(msg) => {
            assert!(msg.contains("400"), "expected status in message: {msg}");
            assert!(
                msg.contains("model not found"),
                "expected error in message: {msg}"
            );
        }
        other => panic!("expected LlmError::Api, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_ollama_list_models_parses_tags_response() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [
                {"name": "m1"},
                {"name": "m2"}
            ]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = OllamaLlmProvider::with_url(mock_server.uri(), None);
    let models = provider.list_models().await.unwrap();
    assert_eq!(models, vec!["m1".to_string(), "m2".to_string()]);
}

#[tokio::test]
async fn test_cohere_complete_sends_expected_request_body() {
    let mock_server = MockServer::start().await;

    let expected_request = json!({
        "model": "command-r-08-2024",
        "stream": false,
        "messages": [
            {"role": "system", "content": "sys"},
            {"role": "user", "content": "user"}
        ],
        "temperature": 0.0,
        "max_tokens": 1024
    });

    Mock::given(method("POST"))
        .and(path("/v2/chat"))
        .and(header("authorization", "Bearer test_key"))
        .and(body_json(&expected_request))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": { "content": [ { "type": "text", "text": "hello from cohere mock" } ] }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = CohereLlmProvider::new("test_key".to_string())
        .with_api_base_url(format!("{}/v2/chat", mock_server.uri()));

    let out = provider.complete("sys", "user").await.unwrap();
    assert_eq!(out, "hello from cohere mock");
}

#[tokio::test]
async fn test_cohere_complete_parses_json_error_body() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v2/chat"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "message": "bad request"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = CohereLlmProvider::new("test_key".to_string())
        .with_api_base_url(format!("{}/v2/chat", mock_server.uri()));

    let err = provider
        .complete("sys", "user")
        .await
        .expect_err("expected error");

    match err {
        LlmError::Api(msg) => {
            assert!(msg.contains("400"), "expected status in message: {msg}");
            assert!(
                msg.contains("bad request"),
                "expected error in message: {msg}"
            );
        }
        other => panic!("expected LlmError::Api, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_fireworks_complete_sends_expected_request_body() {
    let mock_server = MockServer::start().await;

    let expected_request = json!({
        "model": "test-model",
        "messages": [
            {"role": "system", "content": "sys"},
            {"role": "user", "content": "user"}
        ],
        "max_tokens": 4096,
        "temperature": 0.3
    });

    Mock::given(method("POST"))
        .and(path("/inference/v1/chat/completions"))
        .and(header("authorization", "Bearer test_key"))
        .and(body_json(&expected_request))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [
                { "message": { "content": "hello from fireworks mock" } }
            ]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider =
        FireworksLlmProvider::with_model("test_key".to_string(), "test-model".to_string())
            .with_api_base_url(format!(
                "{}/inference/v1/chat/completions",
                mock_server.uri()
            ));

    let out = provider.complete("sys", "user").await.unwrap();
    assert_eq!(out, "hello from fireworks mock");
}

#[tokio::test]
async fn test_fireworks_complete_parses_json_error_body() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/inference/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": { "message": "bad request" }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider =
        FireworksLlmProvider::with_model("test_key".to_string(), "test-model".to_string())
            .with_api_base_url(format!(
                "{}/inference/v1/chat/completions",
                mock_server.uri()
            ));

    let err = provider
        .complete("sys", "user")
        .await
        .expect_err("expected error");

    match err {
        LlmError::Api(msg) => {
            assert!(msg.contains("400"), "expected status in message: {msg}");
            assert!(
                msg.contains("bad request"),
                "expected error in message: {msg}"
            );
        }
        other => panic!("expected LlmError::Api, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_openai_complete_sends_expected_request_body_when_structured_outputs_disabled() {
    let mock_server = MockServer::start().await;

    // When Structured Outputs are disabled, OpenAI uses the simple Responses API request.
    let expected_request = json!({
        "model": "gpt-4o-mini",
        "input": [
            {"role": "system", "content": "sys"},
            {"role": "user", "content": "user"}
        ],
        "max_output_tokens": 4096,
        "temperature": 0.0
    });

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", "Bearer test_key"))
        .and(body_json(&expected_request))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "output_text": "hello from openai mock"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = OpenAiLlmProvider::new("test_key".to_string())
        .with_structured_outputs(false)
        .with_api_base_url(mock_server.uri());

    let out = provider.complete("sys", "user").await.unwrap();
    assert_eq!(out, "hello from openai mock");
}

#[tokio::test]
async fn test_openai_complete_parses_json_error_body() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": { "message": "bad request" }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = OpenAiLlmProvider::new("test_key".to_string())
        .with_structured_outputs(false)
        .with_api_base_url(mock_server.uri());

    let err = provider
        .complete("sys", "user")
        .await
        .expect_err("expected error");

    match err {
        LlmError::Api(msg) => {
            assert!(msg.contains("400"), "expected status in message: {msg}");
            assert!(
                msg.contains("bad request"),
                "expected error in message: {msg}"
            );
        }
        other => panic!("expected LlmError::Api, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_anthropic_complete_sends_expected_headers() {
    let mock_server = MockServer::start().await;

    let guard = Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test_anthropic_key"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": [{"type": "text", "text": "hello from anthropic"}],
            "model": "claude-3-haiku-20240307",
            "role": "assistant"
        })))
        .expect(1)
        .mount_as_scoped(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let provider =
        AnthropicLlmProvider::with_client(client, "test_anthropic_key".to_string(), None)
            .with_api_base_url(format!("{}/v1/messages", mock_server.uri()));

    let result = provider.complete("system prompt", "user message").await;
    assert!(result.is_ok(), "Anthropic complete failed: {:?}", result);

    let received = guard.received_requests().await;
    assert_eq!(received.len(), 1);
}

#[tokio::test]
async fn test_groq_complete_sends_expected_request_body() {
    let mock_server = MockServer::start().await;

    let expected_request = json!({
        "model": "test-model",
        "messages": [
            {"role": "system", "content": "sys"},
            {"role": "user", "content": "user"}
        ],
        "max_tokens": 4096,
        "temperature": 0.3
    });

    Mock::given(method("POST"))
        .and(path("/openai/v1/chat/completions"))
        .and(header("authorization", "Bearer test_key"))
        .and(body_json(&expected_request))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [
                {"message": {"content": "hello from groq"}}
            ]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = GroqLlmProvider::with_model("test_key".to_string(), "test-model".to_string())
        .with_api_base_url(format!("{}/openai/v1/chat/completions", mock_server.uri()));
    let out = provider.complete("sys", "user").await.unwrap();
    assert_eq!(out, "hello from groq");
}

#[tokio::test]
async fn test_groq_complete_parses_json_error_body() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/openai/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": { "message": "invalid api key" }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = GroqLlmProvider::new("test_key".to_string())
        .with_api_base_url(format!("{}/openai/v1/chat/completions", mock_server.uri()));

    let err = provider
        .complete("sys", "user")
        .await
        .expect_err("expected error");

    match err {
        LlmError::Api(msg) => {
            assert!(msg.contains("401"), "expected status in message: {msg}");
            assert!(
                msg.contains("invalid api key"),
                "expected error in message: {msg}"
            );
        }
        other => panic!("expected LlmError::Api, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_cohere_complete_sends_expected_request_body() {
    let mock_server = MockServer::start().await;

    let expected_request = json!({
        "model": "test-model",
        "stream": false,
        "messages": [
            {"role": "system", "content": "sys"},
            {"role": "user", "content": "user"}
        ],
        "temperature": 0.0,
        "max_tokens": 1024
    });

    Mock::given(method("POST"))
        .and(path("/v2/chat"))
        .and(header("authorization", "Bearer test_key"))
        .and(body_json(&expected_request))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": {
                "content": [
                    {"type": "text", "text": "hello from cohere"}
                ]
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = CohereLlmProvider::with_model("test_key".to_string(), "test-model".to_string())
        .with_api_base_url(format!("{}/v2/chat", mock_server.uri()));
    let out = provider.complete("sys", "user").await.unwrap();
    assert_eq!(out, "hello from cohere");
}

#[tokio::test]
async fn test_cohere_complete_parses_json_error_body() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v2/chat"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "message": "quota exceeded"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let provider = CohereLlmProvider::new("test_key".to_string())
        .with_api_base_url(format!("{}/v2/chat", mock_server.uri()));

    let err = provider
        .complete("sys", "user")
        .await
        .expect_err("expected error");

    match err {
        LlmError::Api(msg) => {
            assert!(msg.contains("400"), "expected status in message: {msg}");
            assert!(
                msg.contains("quota exceeded"),
                "expected error in message: {msg}"
            );
        }
        other => panic!("expected LlmError::Api, got: {other:?}"),
    }
}
