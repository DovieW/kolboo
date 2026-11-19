//! Bounded HTTP framing. Protocol interpretation stays in each adapter;
//! dropping the future closes the body without spawning detached work.
use super::LlmError;
use futures_util::StreamExt;
use reqwest::RequestBuilder;
use std::time::Duration;

pub(crate) type Delta<'a> = &'a mut (dyn FnMut(&str) + Send);
const MAX_FRAME: usize = 256 * 1024;
const MAX_BODY: usize = 8 * 1024 * 1024;
const MAX_TEXT: usize = 1024 * 1024;

#[derive(Default)]
pub(crate) struct Output {
    pub(crate) text: String,
    pub(crate) usage: Option<serde_json::Value>,
}

impl Output {
    pub(crate) fn push(&mut self, delta: &str, notify: Delta<'_>) -> Result<(), LlmError> {
        if self.text.len().saturating_add(delta.len()) > MAX_TEXT {
            return Err(invalid("Answer exceeds the supported size"));
        }
        if !delta.is_empty() {
            self.text.push_str(delta);
            notify(&self.text);
        }
        Ok(())
    }
    pub(crate) fn finish(
        self,
        store: Option<&crate::request_log::RequestLogStore>,
    ) -> Result<String, LlmError> {
        if self.text.trim().is_empty() {
            return Err(invalid("Provider returned no answer text"));
        }
        if let Some(store) = store {
            store.with_current(|log| log.llm_response_json = self.usage);
        }
        Ok(self.text)
    }
}

pub(crate) fn invalid(message: &str) -> LlmError {
    LlmError::InvalidResponse(message.into())
}
pub(crate) fn json(data: &str) -> Result<serde_json::Value, LlmError> {
    serde_json::from_str(data).map_err(|_| invalid("Malformed streaming response"))
}

/// Keep only numeric billing fields, never provider content or errors.
pub(crate) fn usage(value: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    let mut clean = serde_json::Map::new();
    for key in [
        "prompt_tokens",
        "completion_tokens",
        "total_tokens",
        "input_tokens",
        "output_tokens",
        "cached_tokens",
        "reasoning_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
        "ephemeral_5m_input_tokens",
        "ephemeral_1h_input_tokens",
        "promptTokenCount",
        "candidatesTokenCount",
        "thoughtsTokenCount",
        "totalTokenCount",
        "prompt_eval_count",
        "eval_count",
    ] {
        if let Some(number) = value.get(key).and_then(serde_json::Value::as_u64) {
            clean.insert(key.into(), number.into());
        }
    }
    for key in [
        "input_tokens_details",
        "output_tokens_details",
        "cache_creation",
        "tokens",
        "billed_units",
    ] {
        if let Some(child) = value.get(key) {
            clean.insert(key.into(), serde_json::Value::Object(usage(child)));
        }
    }
    clean
}

pub(crate) async fn read(
    request: RequestBuilder,
    timeout: Option<Duration>,
    sse: bool,
    consume: &mut (dyn FnMut(&str) -> Result<bool, LlmError> + Send),
) -> Result<(), LlmError> {
    let timeout = timeout.unwrap_or(Duration::from_secs(120));
    let request = crate::http::with_cloudflare_access_headers_from_request_url(request);
    let operation = async {
        let response = request
            .send()
            .await
            .map_err(|_| LlmError::Api("Could not connect to the model provider".into()))?;
        if !response.status().is_success() {
            return Err(LlmError::Api(format!(
                "Model provider returned HTTP {}",
                response.status().as_u16()
            )));
        }
        let mut body = response.bytes_stream();
        let mut frames = Frames::new(sse);
        let mut total = 0usize;
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|_| invalid("Answer stream was interrupted"))?;
            total = total.saturating_add(chunk.len());
            if total > MAX_BODY {
                return Err(invalid("Response exceeds the supported size"));
            }
            for frame in frames.feed(&chunk)? {
                if consume(&frame)? {
                    return Ok(());
                }
            }
        }
        // NDJSON permits a final line without a newline; SSE needs its delimiter.
        if !sse && !frames.pending.is_empty() {
            let frame = std::str::from_utf8(&frames.pending)
                .map_err(|_| invalid("Invalid UTF-8 response"))?;
            if consume(frame)? {
                return Ok(());
            }
        }
        Err(invalid("Answer stream ended before completion"))
    };
    tokio::time::timeout(timeout, operation)
        .await
        .map_err(|_| LlmError::Timeout(timeout))?
}

struct Frames {
    sse: bool,
    pending: Vec<u8>,
    data: String,
}
impl Frames {
    fn new(sse: bool) -> Self {
        Self {
            sse,
            pending: vec![],
            data: String::new(),
        }
    }
    fn feed(&mut self, chunk: &[u8]) -> Result<Vec<String>, LlmError> {
        let mut result = Vec::new();
        for &byte in chunk {
            self.pending.push(byte);
            if self.pending.len() + self.data.len() > MAX_FRAME {
                return Err(invalid("Streaming frame exceeds the supported size"));
            }
            if byte != b'\n' {
                continue;
            }
            let line = std::str::from_utf8(&self.pending)
                .map_err(|_| invalid("Invalid UTF-8 response"))?
                .trim_end_matches(['\n', '\r']);
            if !self.sse {
                if !line.is_empty() {
                    result.push(line.to_string());
                }
            } else if line.is_empty() {
                if !self.data.is_empty() {
                    result.push(std::mem::take(&mut self.data));
                }
            } else if let Some(data) = line.strip_prefix("data:") {
                if !self.data.is_empty() {
                    self.data.push('\n');
                }
                self.data.push_str(data.strip_prefix(' ').unwrap_or(data));
            }
            self.pending.clear();
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
