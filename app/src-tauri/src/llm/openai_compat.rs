//! Shared helpers for OpenAI-compatible Chat Completions APIs.
//!
//! Several providers (Groq, Fireworks, Cerebras, etc.) expose OpenAI-compatible
//! endpoints with the same request/response envelope. This module centralizes
//! the "common shape" so individual provider modules can focus on:
//! - provider-specific base URL / auth / headers
//! - provider-specific optional parameters
//! - provider-specific edge-case handling

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub(super) struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: &str) -> Self {
        Self {
            role: "system".to_string(),
            content: content.to_string(),
        }
    }

    pub fn user(content: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: content.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl ChatRequest {
    pub fn new(
        model: String,
        system_prompt: &str,
        user_message: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Self {
        Self {
            model,
            messages: vec![
                ChatMessage::system(system_prompt),
                ChatMessage::user(user_message),
            ],
            max_tokens,
            temperature,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Deserialize)]
pub(super) struct ErrorDetail {
    pub message: String,
}

fn extract_content_from_choice(choice: &serde_json::Value) -> Option<String> {
    // OpenAI Chat Completions: choices[0].message.content is typically a string.
    if let Some(s) = choice
        .get("message")
        .and_then(|msg| msg.get("content"))
        .and_then(|c| c.as_str())
    {
        return Some(s.to_string());
    }

    // Some OpenAI-compatible implementations may return content as an array of parts,
    // e.g. [{"type":"text","text":"..."}, ...]. Join text parts.
    if let Some(parts) = choice
        .get("message")
        .and_then(|msg| msg.get("content"))
        .and_then(|c| c.as_array())
    {
        let mut out = String::new();
        for part in parts {
            if let Some(text) = part
                .get("text")
                .and_then(|t| t.as_str())
                .or_else(|| part.get("content").and_then(|t| t.as_str()))
            {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(text);
            }
        }
        if !out.trim().is_empty() {
            return Some(out);
        }
    }

    // Legacy completions-style: choices[0].text
    if let Some(s) = choice.get("text").and_then(|t| t.as_str()) {
        return Some(s.to_string());
    }

    None
}

/// Extract the first chat completion content from an OpenAI-compatible response.
///
/// This intentionally handles a few known variants:
/// - `choices[0].message.content` as a string (OpenAI default)
/// - `choices[0].message.content` as an array of parts
/// - `choices[0].text` (legacy completions)
pub(super) fn extract_first_choice_text(response_json: &serde_json::Value) -> Option<String> {
    let choices = response_json.get("choices")?.as_array()?;
    let first = choices.first()?;
    extract_content_from_choice(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_basic_chat_request() {
        let req = ChatRequest::new("m".to_string(), "sys", "user", 7, 0.3);
        let v = serde_json::to_value(&req).expect("serialize");
        assert_eq!(v["model"], "m");
        assert_eq!(v["max_tokens"], 7);
        assert_eq!(v["temperature"], 0.3);
        assert_eq!(v["messages"][0]["role"], "system");
        assert_eq!(v["messages"][0]["content"], "sys");
        assert_eq!(v["messages"][1]["role"], "user");
        assert_eq!(v["messages"][1]["content"], "user");
    }

    #[test]
    fn extracts_string_content() {
        let response = json!({
            "choices": [{
                "message": {"content": "hello"}
            }]
        });
        assert_eq!(
            extract_first_choice_text(&response),
            Some("hello".to_string())
        );
    }

    #[test]
    fn extracts_array_parts_content() {
        let response = json!({
            "choices": [{
                "message": {
                    "content": [
                        {"type": "text", "text": "a"},
                        {"type": "text", "text": "b"}
                    ]
                }
            }]
        });
        assert_eq!(
            extract_first_choice_text(&response),
            Some("a\nb".to_string())
        );
    }

    #[test]
    fn extracts_legacy_text_field() {
        let response = json!({
            "choices": [{"text": "legacy"}]
        });
        assert_eq!(
            extract_first_choice_text(&response),
            Some("legacy".to_string())
        );
    }
}
