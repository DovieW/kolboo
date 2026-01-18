use schemars::schema_for;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[test]
fn history_page_query_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::history::HistoryPageQuery);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated HistoryPageQuery schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("history-page-query.schema.json");

    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Missing schema file: {}", path.display()));
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
    let checked_in: Value =
        serde_json::from_str(raw).expect("Invalid JSON in history-page-query.schema.json");

    assert_eq!(
		generated, checked_in,
		"HistoryPageQuery schema changed. Regenerate history-page-query.schema.json using the export_history_page_query_schema bin.",
	);
}

#[test]
fn history_page_result_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::history::HistoryPageResult);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated HistoryPageResult schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("history-page-result.schema.json");

    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Missing schema file: {}", path.display()));
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
    let checked_in: Value =
        serde_json::from_str(raw).expect("Invalid JSON in history-page-result.schema.json");

    assert_eq!(
		generated, checked_in,
		"HistoryPageResult schema changed. Regenerate history-page-result.schema.json using the export_history_page_result_schema bin.",
	);
}
