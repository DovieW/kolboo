//! Kolboo Managed LLM adapter.
//!
//! The desktop sends an OpenAI-compatible Chat Completions request to API Edge.
//! API Edge owns authentication, authorization, model allowlisting, metering, and
//! Cloudflare AI Gateway routing. Provider credentials never enter the desktop.

use super::http_json;
use super::openai_compat::{self, ChatMessage};
use super::{LlmError, LlmProvider, DEFAULT_LLM_TIMEOUT};
use crate::request_log::RequestLogStore;
use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use std::time::Duration;

const DEFAULT_MODEL: &str = "gpt-4o-mini";

#[derive(Debug, Serialize)]
struct ManagedChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

impl ManagedChatRequest {
    fn new(model: String, system_prompt: &str, user_message: &str) -> Self {
        Self {
            model,
            messages: vec![
                ChatMessage::system(system_prompt),
                ChatMessage::user(user_message),
            ],
        }
    }
}

fn parse_managed_error_message(body: &str) -> Option<String> {
    let payload = serde_json::from_str::<serde_json::Value>(body).ok()?;
    payload
        .get("message")
        .and_then(|value| value.as_str())
        .or_else(|| {
            payload
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(|value| value.as_str())
        })
        .map(str::to_string)
}

pub struct ManagedLlmProvider {
    client: Client,
    access_token: String,
    model: String,
    api_url: String,
    timeout: Option<Duration>,
    request_log_store: Option<RequestLogStore>,
}

impl ManagedLlmProvider {
    pub fn with_client(
        client: Client,
        access_token: String,
        model: Option<String>,
        api_url: String,
    ) -> Self {
        Self {
            client,
            access_token,
            model: model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            api_url,
            timeout: Some(DEFAULT_LLM_TIMEOUT),
            request_log_store: None,
        }
    }

    pub fn new(access_token: String, model: Option<String>, api_url: String) -> Self {
        Self::with_client(Client::new(), access_token, model, api_url)
    }

    pub fn with_request_log_store(mut self, store: Option<RequestLogStore>) -> Self {
        self.request_log_store = store;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn without_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }
}

#[async_trait]
impl LlmProvider for ManagedLlmProvider {
    async fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String, LlmError> {
        if self.access_token.trim().is_empty() {
            return Err(LlmError::NoApiKey("managed session".to_string()));
        }
        if self.api_url.trim().is_empty() {
            return Err(LlmError::ProviderNotAvailable(
                "managed inference gateway is not configured".to_string(),
            ));
        }

        let request = ManagedChatRequest::new(self.model.clone(), system_prompt, user_message);
        let idempotency_key = format!("desktop-llm-{}", uuid::Uuid::new_v4());
        let req = self
            .client
            .post(&self.api_url)
            .bearer_auth(&self.access_token)
            .header("x-idempotency-key", idempotency_key)
            .json(&request);

        let response_json = http_json::send_json_request_logged_with_error_parser(
            "Kolboo Managed",
            "managed",
            req,
            self.timeout,
            self.request_log_store.as_ref(),
            &request,
            parse_managed_error_message,
        )
        .await?;

        openai_compat::extract_first_choice_text(&response_json)
            .ok_or_else(|| LlmError::InvalidResponse("No response choices returned".to_string()))
    }

    fn name(&self) -> &'static str {
        "managed"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_safe_default_model() {
        let provider = ManagedLlmProvider::new(
            "session-token".to_string(),
            None,
            "https://gateway.example/v1/chat/completions".to_string(),
        );
        assert_eq!(provider.name(), "managed");
        assert_eq!(provider.model(), DEFAULT_MODEL);
    }

    #[test]
    fn parses_stable_api_edge_errors() {
        assert_eq!(
            parse_managed_error_message(
                r#"{"code":"MODEL_NOT_SUPPORTED","message":"Choose another model."}"#,
            ),
            Some("Choose another model.".to_string())
        );
    }
}
