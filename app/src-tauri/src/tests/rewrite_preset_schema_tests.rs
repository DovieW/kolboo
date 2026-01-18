use schemars::schema_for;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[test]
fn rewrite_preset_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::settings::RewritePreset);
    let generated: Value =
        serde_json::to_value(schema).expect("Failed to serialize generated RewritePreset schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("rewrite-preset.schema.json");

    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Missing schema file: {}", path.display()));
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
    let checked_in: Value =
        serde_json::from_str(raw).expect("Invalid JSON in rewrite-preset.schema.json");

    assert_eq!(
		generated, checked_in,
		"RewritePreset schema changed. Regenerate rewrite-preset.schema.json using the export_rewrite_preset_schema bin.",
	);
}
