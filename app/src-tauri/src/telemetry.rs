//! Telemetry Mapping helpers.
//!
//! `request_log` owns storage, redaction, and the rich debugging shape. This
//! Module owns the smaller read models that other systems consume from a
//! `RequestLog`, so those systems do not need to understand every request kind.

use crate::request_log::{RequestKind, RequestLog};
use serde_json::Value as JsonValue;

/// Cost Reporting input extracted from a request log.
///
/// Keep this intentionally narrow: Stats only needs request identity, provider
/// metadata, and provider responses. User text and request payloads should stay
/// behind `request_log` redaction/export helpers unless a caller proves it needs
/// them.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CostTelemetryInputs {
    pub request_id: String,
    pub stt_provider: String,
    pub stt_model: Option<String>,
    pub stt_response_json: Option<JsonValue>,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub llm_response_json: Option<JsonValue>,
}

/// Map the rich `RequestLog` shape into the compact Cost Reporting shape.
///
/// Quick Ask and Quick Replace have their own LLM call metadata. Prefer those
/// fields for cost reporting, but keep the legacy/base LLM fields as fallbacks
/// so older logs and partially-completed flows still produce the best available
/// cost event.
pub(crate) fn cost_inputs_from_request_log(log: &RequestLog) -> CostTelemetryInputs {
    let (llm_provider, llm_model, llm_response_json) = llm_fields_for_cost(log);

    CostTelemetryInputs {
        request_id: log.id.clone(),
        stt_provider: log.stt_provider.clone(),
        stt_model: log.stt_model.clone(),
        stt_response_json: log.stt_response_json.clone(),
        llm_provider,
        llm_model,
        llm_response_json,
    }
}

fn llm_fields_for_cost(log: &RequestLog) -> (Option<String>, Option<String>, Option<JsonValue>) {
    match log.kind {
        RequestKind::QuickAsk => (
            log.quick_ask_provider
                .clone()
                .or_else(|| log.llm_provider.clone()),
            log.quick_ask_model
                .clone()
                .or_else(|| log.llm_model.clone()),
            log.quick_ask_response_json
                .clone()
                .or_else(|| log.llm_response_json.clone()),
        ),
        RequestKind::QuickReplace => (
            log.quick_replace_provider
                .clone()
                .or_else(|| log.llm_provider.clone()),
            log.quick_replace_model
                .clone()
                .or_else(|| log.llm_model.clone()),
            log.quick_replace_response_json
                .clone()
                .or_else(|| log.llm_response_json.clone()),
        ),
        RequestKind::Transcription => (
            log.llm_provider.clone(),
            log.llm_model.clone(),
            log.llm_response_json.clone(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_log(kind: RequestKind) -> RequestLog {
        let mut log = RequestLog::new("openai".to_string(), Some("whisper-1".to_string()));
        log.kind = kind;
        log.llm_provider = Some("openai".to_string());
        log.llm_model = Some("gpt-4o-mini".to_string());
        log.llm_response_json = Some(serde_json::json!({ "usage": { "input_tokens": 1 } }));
        log.stt_response_json =
            Some(serde_json::json!({ "usage": { "type": "duration", "seconds": 3 } }));
        log
    }

    #[test]
    fn transcription_cost_mapping_uses_base_llm_fields() {
        let log = base_log(RequestKind::Transcription);

        let inputs = cost_inputs_from_request_log(&log);

        assert_eq!(inputs.stt_provider, "openai");
        assert_eq!(inputs.stt_model.as_deref(), Some("whisper-1"));
        assert_eq!(inputs.llm_provider.as_deref(), Some("openai"));
        assert_eq!(inputs.llm_model.as_deref(), Some("gpt-4o-mini"));
        assert_eq!(inputs.llm_response_json, log.llm_response_json);
    }

    #[test]
    fn quick_ask_cost_mapping_prefers_quick_action_llm_fields() {
        let mut log = base_log(RequestKind::QuickAsk);
        log.quick_ask_provider = Some("anthropic".to_string());
        log.quick_ask_model = Some("claude-sonnet".to_string());
        log.quick_ask_response_json = Some(serde_json::json!({ "usage": { "input_tokens": 9 } }));

        let inputs = cost_inputs_from_request_log(&log);

        assert_eq!(inputs.llm_provider.as_deref(), Some("anthropic"));
        assert_eq!(inputs.llm_model.as_deref(), Some("claude-sonnet"));
        assert_eq!(inputs.llm_response_json, log.quick_ask_response_json);
    }

    #[test]
    fn quick_replace_cost_mapping_falls_back_to_base_llm_fields() {
        let log = base_log(RequestKind::QuickReplace);

        let inputs = cost_inputs_from_request_log(&log);

        assert_eq!(inputs.llm_provider.as_deref(), Some("openai"));
        assert_eq!(inputs.llm_model.as_deref(), Some("gpt-4o-mini"));
        assert_eq!(inputs.llm_response_json, log.llm_response_json);
    }
}
