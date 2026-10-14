//! Telemetry Mapping helpers.
//!
//! `request_log` owns storage, redaction, and the rich debugging shape. This
//! Module owns the smaller read models that other systems consume from a
//! `RequestLog`, so those systems do not need to understand every request kind.

use crate::request_log::{RequestKind, RequestLog};
use serde_json::Value as JsonValue;

/// Finite, code-reviewed taxonomy; arbitrary provider strings stay unknown.
pub(crate) fn analytics_language(value: &str) -> Option<&'static str> {
    match value {
        "english" => return Some("en"),
        "yiddish" => return Some("yi"),
        "hebrew" => return Some("he"),
        "spanish" => return Some("es"),
        "french" => return Some("fr"),
        "german" => return Some("de"),
        "italian" => return Some("it"),
        "portuguese" => return Some("pt"),
        "chinese" => return Some("zh"),
        "japanese" => return Some("ja"),
        "korean" => return Some("ko"),
        "hindi" => return Some("hi"),
        "arabic" => return Some("ar"),
        "russian" => return Some("ru"),
        "iw" => return Some("he"),
        _ => {}
    }
    static CODES: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!("../../src/lib/telemetry/languageCodes.json"))
            .expect("Bundled analytics language taxonomy must be valid JSON")
    });
    CODES.iter().copied().find(|code| *code == value)
}

pub(crate) fn provider_reported_languages(response: &JsonValue) -> Vec<String> {
    let mut codes = Vec::new();
    // Read language metadata only. No transcript text, prompts or extra requests.
    let values = [
        response.get("language"),
        response.get("language_code"),
        response.pointer("/metadata/transcript_data/language"),
    ];
    for value in values.into_iter().flatten().chain(
        response
            .pointer("/results/channels")
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
            .filter_map(|channel| channel.get("detected_language")),
    ) {
        if let Some(code) = value
            .as_str()
            .and_then(|text| analytics_language(&text.trim().to_lowercase()))
        {
            if !codes.iter().any(|existing| existing == code) {
                codes.push(code.to_owned());
            }
        }
    }
    codes
}

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

    #[test]
    fn language_metadata_is_finite_content_free_and_accumulates_across_chunks() {
        for (name, code) in [
            ("english", "en"),
            ("yiddish", "yi"),
            ("hebrew", "he"),
            ("spanish", "es"),
            ("french", "fr"),
            ("german", "de"),
            ("italian", "it"),
            ("portuguese", "pt"),
            ("chinese", "zh"),
            ("japanese", "ja"),
            ("korean", "ko"),
            ("hindi", "hi"),
            ("arabic", "ar"),
            ("russian", "ru"),
            ("iw", "he"),
        ] {
            assert_eq!(analytics_language(name), Some(code));
            assert_eq!(analytics_language(code), Some(code));
        }
        assert_eq!(analytics_language("customer words"), None);
        let mut log = RequestLog::new("assemblyai".into(), None);
        log.set_stt_response(serde_json::json!({"language_code":"yi", "text":"private text"}));
        log.set_stt_response(serde_json::json!({"language":"Hebrew", "prompt":"private text"}));
        log.set_stt_response(serde_json::json!({"metadata":{"transcript_data":{"language":"en"}}, "results":{"channels":[{"detected_language":"yi"},{"detected_language":"he"},{"detected_language":"private"}]}}));
        log.set_stt_response(serde_json::json!({"language":4, "language_code":"private transcript", "text":"English"}));
        assert_eq!(log.stt_detected_languages, ["yi", "he", "en"]);
        assert!(provider_reported_languages(
            &serde_json::json!({"text":"English", "language":null})
        )
        .is_empty());
        let retained: RequestLog =
            serde_json::from_value(serde_json::to_value(&log).unwrap()).unwrap();
        assert_eq!(retained.stt_detected_languages, log.stt_detected_languages);
        let mut legacy = serde_json::to_value(&log).unwrap();
        legacy.as_object_mut().unwrap().remove("stt_language_hint");
        legacy
            .as_object_mut()
            .unwrap()
            .remove("stt_detected_languages");
        let restored: RequestLog = serde_json::from_value(legacy).unwrap();
        assert!(restored.stt_detected_languages.is_empty());
        assert_eq!(restored.stt_language_hint, None);
    }

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
