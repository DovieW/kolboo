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
fn default_sections_response_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::config::DefaultSectionsResponse);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated DefaultSectionsResponse schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("default-sections-response.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"DefaultSectionsResponse schema changed. Regenerate default-sections-response.schema.json using the export_default_sections_response_schema bin.",
	);
}

#[test]
fn available_providers_response_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::config::AvailableProvidersResponse);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated AvailableProvidersResponse schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("available-providers-response.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"AvailableProvidersResponse schema changed. Regenerate available-providers-response.schema.json using the export_available_providers_response_schema bin.",
	);
}

#[test]
fn history_delete_options_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::history::HistoryDeleteOptions);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated HistoryDeleteOptions schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("history-delete-options.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"HistoryDeleteOptions schema changed. Regenerate history-delete-options.schema.json using the export_history_delete_options_schema bin.",
	);
}

#[test]
fn history_delete_result_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::history::HistoryDeleteResult);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated HistoryDeleteResult schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("history-delete-result.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"HistoryDeleteResult schema changed. Regenerate history-delete-result.schema.json using the export_history_delete_result_schema bin.",
	);
}

#[test]
fn history_delete_mode_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::history::HistoryDeleteMode);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated HistoryDeleteMode schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("history-delete-mode.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"HistoryDeleteMode schema changed. Regenerate history-delete-mode.schema.json using the export_history_delete_mode_schema bin.",
	);
}
