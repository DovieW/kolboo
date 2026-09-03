use schemars::schema_for;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[test]
fn request_log_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::request_log::RequestLog);
    let generated: Value =
        serde_json::to_value(schema).expect("Failed to serialize generated RequestLog schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("request-log.schema.json");

    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Missing schema file: {}", path.display()));
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
    let checked_in: Value =
        serde_json::from_str(raw).expect("Invalid JSON in request-log.schema.json");

    assert_eq!(
        generated, checked_in,
        "RequestLog schema changed. Regenerate schemas with `pnpm -C app schemas:generate`.",
    );
}

#[test]
fn request_log_preserves_strategy_independent_router_decision_diagnostics() {
    let mut log = crate::request_log::RequestLog::new("mock-stt".to_string(), None);
    log.router_strategy = Some("embeddings".to_string());
    log.router_response_json = Some(serde_json::json!({
        "type": "embeddings",
        "outcome": "ambiguous",
        "selected_preset_id": null,
        "scores": [["email", 0.91], ["calendar", 0.88]],
    }));

    let value = serde_json::to_value(&log).expect("request log serializes");

    assert_eq!(value["router_strategy"], "embeddings");
    assert_eq!(value["router_response_json"]["outcome"], "ambiguous");
    assert_eq!(
        value["router_response_json"]["selected_preset_id"],
        Value::Null
    );
}

#[test]
fn request_log_redaction_preserves_provider_metadata_but_removes_secrets_and_payloads() {
    let mut log =
        crate::request_log::RequestLog::new("openai".to_string(), Some("whisper-1".to_string()));
    log.llm_provider = Some("anthropic".to_string());
    log.llm_model = Some("claude-sonnet".to_string());
    log.formatted_transcript = Some("private transcript".to_string());
    log.stt_request_json = Some(serde_json::json!({
        "Authorization": "Bearer secret-token",
        "model": "whisper-1",
        "metadata": { "safe": "kept" }
    }));

    let redacted =
        crate::request_log::redact_json(log.stt_request_json.clone().expect("request json"));
    assert_eq!(redacted["Authorization"], "<redacted>");
    assert_eq!(redacted["model"], "whisper-1");
    assert_eq!(redacted["metadata"]["safe"], "kept");

    let stripped = crate::request_log::strip_request_log_text_and_payloads(log);
    assert_eq!(stripped.stt_provider, "openai");
    assert_eq!(stripped.stt_model.as_deref(), Some("whisper-1"));
    assert_eq!(stripped.llm_provider.as_deref(), Some("anthropic"));
    assert_eq!(stripped.llm_model.as_deref(), Some("claude-sonnet"));
    assert_eq!(stripped.formatted_transcript, None);
    assert_eq!(stripped.stt_request_json, None);
}
