use schemars::schema_for;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[test]
fn intent_router_settings_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::settings::IntentRouterSettings);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated IntentRouterSettings schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("intent-router-settings.schema.json");

    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Missing schema file: {}", path.display()));
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
    let checked_in: Value =
        serde_json::from_str(raw).expect("Invalid JSON in intent-router-settings.schema.json");

    assert_eq!(
		generated, checked_in,
		"IntentRouterSettings schema changed. Regenerate intent-router-settings.schema.json using the export_intent_router_settings_schema bin.",
	);
}
