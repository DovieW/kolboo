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
fn audio_capture_diagnostics_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::audio_capture::AudioCaptureDiagnostics);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated AudioCaptureDiagnostics schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("audio-capture-diagnostics.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"AudioCaptureDiagnostics schema changed. Regenerate audio-capture-diagnostics.schema.json using the export_audio_capture_diagnostics_schema bin.",
	);
}

#[test]
fn audio_level_stats_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::audio_capture::AudioLevelStats);
    let generated: Value =
        serde_json::to_value(schema).expect("Failed to serialize generated AudioLevelStats schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("audio-level-stats.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"AudioLevelStats schema changed. Regenerate audio-level-stats.schema.json using the export_audio_level_stats_schema bin.",
	);
}

#[test]
fn recordings_stats_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::recordings::RecordingsStats);
    let generated: Value =
        serde_json::to_value(schema).expect("Failed to serialize generated RecordingsStats schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("recordings-stats.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"RecordingsStats schema changed. Regenerate recordings-stats.schema.json using the export_recordings_stats_schema bin.",
	);
}

#[test]
fn data_storage_summary_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::data::DataStorageSummary);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated DataStorageSummary schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("data-storage-summary.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"DataStorageSummary schema changed. Regenerate data-storage-summary.schema.json using the export_data_storage_summary_schema bin.",
	);
}
