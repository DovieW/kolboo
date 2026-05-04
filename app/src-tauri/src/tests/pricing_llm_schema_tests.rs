use schemars::schema_for;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn read_schema(path: &PathBuf) -> Value {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("Missing schema file: {}", path.display()));
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
    serde_json::from_str(raw).expect("Invalid JSON schema")
}

#[test]
fn cost_summary_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::stats::CostSummaryResponse);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated CostSummaryResponse schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("cost-summary.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"CostSummaryResponse schema changed. Regenerate cost-summary.schema.json using the export_cost_summary_schema bin.",
	);
}

#[test]
fn cost_by_provider_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::stats::CostByProviderResponse);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated CostByProviderResponse schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("cost-by-provider.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"CostByProviderResponse schema changed. Regenerate cost-by-provider.schema.json using the export_cost_by_provider_schema bin.",
	);
}

#[test]
fn model_pricing_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::pricing::ModelPricingResponse);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated ModelPricingResponse schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("model-pricing.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"ModelPricingResponse schema changed. Regenerate model-pricing.schema.json using the export_model_pricing_schema bin.",
	);
}

#[test]
fn cache_router_embeddings_response_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::router::CacheRouterEmbeddingsResponse);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated CacheRouterEmbeddingsResponse schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("cache-router-embeddings-response.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"CacheRouterEmbeddingsResponse schema changed. Regenerate cache-router-embeddings-response.schema.json using the export_cache_router_embeddings_response_schema bin.",
	);
}

#[test]
fn open_window_info_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::windows_apps::OpenWindowInfo);
    let generated: Value =
        serde_json::to_value(schema).expect("Failed to serialize generated OpenWindowInfo schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("open-window-info.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"OpenWindowInfo schema changed. Regenerate open-window-info.schema.json using the export_open_window_info_schema bin.",
	);
}

#[test]
fn model_option_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::fireworks::ModelOption);
    let generated: Value =
        serde_json::to_value(schema).expect("Failed to serialize generated ModelOption schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("model-option.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"ModelOption schema changed. Regenerate model-option.schema.json using the export_model_option_schema bin.",
	);
}

#[test]
fn llm_provider_info_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::llm::LlmProviderInfo);
    let generated: Value =
        serde_json::to_value(schema).expect("Failed to serialize generated LlmProviderInfo schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("llm-provider-info.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"LlmProviderInfo schema changed. Regenerate llm-provider-info.schema.json using the export_llm_provider_info_schema bin.",
	);
}

#[test]
fn test_llm_rewrite_response_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::llm::TestLlmRewriteResponse);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated TestLlmRewriteResponse schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("test-llm-rewrite-response.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"TestLlmRewriteResponse schema changed. Regenerate test-llm-rewrite-response.schema.json using the export_test_llm_rewrite_response_schema bin.",
	);
}

#[test]
fn iterate_rewrite_prompt_response_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::llm::IterateRewritePromptResponse);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated IterateRewritePromptResponse schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("iterate-rewrite-prompt-response.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"IterateRewritePromptResponse schema changed. Regenerate iterate-rewrite-prompt-response.schema.json using the export_iterate_rewrite_prompt_response_schema bin.",
	);
}

#[test]
fn test_rewrite_with_prompt_response_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::llm::TestRewriteWithPromptResponse);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated TestRewriteWithPromptResponse schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("test-rewrite-with-prompt-response.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"TestRewriteWithPromptResponse schema changed. Regenerate test-rewrite-with-prompt-response.schema.json using the export_test_rewrite_with_prompt_response_schema bin.",
	);
}

#[test]
fn llm_complete_response_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::llm::LlmCompleteResponse);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated LlmCompleteResponse schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("llm-complete-response.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"LlmCompleteResponse schema changed. Regenerate llm-complete-response.schema.json using the export_llm_complete_response_schema bin.",
	);
}

#[test]
fn provider_family_cost_estimators_preserve_provider_specific_rates() {
    let usage = crate::cost::openai::OpenAiUsage {
        input_tokens: 1_000,
        output_tokens: 500,
        cached_input_tokens: 0,
        input_audio_tokens: 0,
        output_audio_tokens: 0,
    };

    let openai = crate::cost::openai::estimate_cost_from_usage("gpt-4o-mini", usage)
        .expect("openai estimate")
        .total_usd_micros;
    let groq = crate::cost::groq::estimate_llm_cost_from_usage("llama-3.1-8b-instant", usage)
        .expect("groq estimate");
    let fireworks = crate::cost::fireworks::estimate_llm_cost_from_usage(
        "accounts/fireworks/models/llama-v3p1-8b-instruct",
        usage,
    )
    .expect("fireworks estimate");

    assert!(openai > 0);
    assert!(groq > 0);
    assert!(fireworks > 0);
    assert_ne!(openai, groq);
    assert_ne!(groq, fireworks);

    assert!(
        crate::cost::openai::estimate_transcription_cost_from_audio_secs("whisper-1", 60.0)
            .is_some()
    );
    assert!(
        crate::cost::groq::estimate_stt_cost_from_audio_secs("whisper-large-v3-turbo", 60.0)
            .is_some()
    );
}
