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

fn assert_schema_matches_file(generated: Value, file: &str, label: &str) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push(file);

    let checked_in = read_schema(&path);
    assert_eq!(
        generated, checked_in,
        "{label} schema changed. Regenerate {file} using the appropriate export bin.",
    );
}

#[test]
fn system_event_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::SystemEvent);
    let generated: Value =
        serde_json::to_value(schema).expect("Failed to serialize generated SystemEvent schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("system-event.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"SystemEvent schema changed. Regenerate system-event.schema.json using the export_system_event_schema bin.",
	);
}

#[test]
fn pipeline_error_payload_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::PipelineErrorPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated PipelineErrorPayload schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("pipeline-error-payload.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"PipelineErrorPayload schema changed. Regenerate pipeline-error-payload.schema.json using the export_pipeline_error_payload_schema bin.",
	);
}

#[test]
fn pipeline_state_event_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::PipelineStateEvent);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated PipelineStateEvent schema");
    assert_schema_matches_file(
        generated,
        "pipeline-state-changed.schema.json",
        "PipelineStateEvent",
    );
}

#[test]
fn pipeline_transcript_ready_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::PipelineTranscriptReadyPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated PipelineTranscriptReadyPayload schema");
    assert_schema_matches_file(
        generated,
        "pipeline-transcript-ready.schema.json",
        "PipelineTranscriptReadyPayload",
    );
}

#[test]
fn pipeline_recording_started_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::EmptyEventPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated EmptyEventPayload schema");
    assert_schema_matches_file(
        generated,
        "pipeline-recording-started.schema.json",
        "pipeline-recording-started",
    );
}

#[test]
fn pipeline_transcription_started_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::EmptyEventPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated EmptyEventPayload schema");
    assert_schema_matches_file(
        generated,
        "pipeline-transcription-started.schema.json",
        "pipeline-transcription-started",
    );
}

#[test]
fn pipeline_routing_started_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::EmptyEventPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated EmptyEventPayload schema");
    assert_schema_matches_file(
        generated,
        "pipeline-routing-started.schema.json",
        "pipeline-routing-started",
    );
}

#[test]
fn pipeline_rewriting_started_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::EmptyEventPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated EmptyEventPayload schema");
    assert_schema_matches_file(
        generated,
        "pipeline-rewriting-started.schema.json",
        "pipeline-rewriting-started",
    );
}

#[test]
fn pipeline_cancelled_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::EmptyEventPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated EmptyEventPayload schema");
    assert_schema_matches_file(
        generated,
        "pipeline-cancelled.schema.json",
        "pipeline-cancelled",
    );
}

#[test]
fn pipeline_reset_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::EmptyEventPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated EmptyEventPayload schema");
    assert_schema_matches_file(generated, "pipeline-reset.schema.json", "pipeline-reset");
}

#[test]
fn recording_start_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::EmptyEventPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated EmptyEventPayload schema");
    assert_schema_matches_file(generated, "recording-start.schema.json", "recording-start");
}

#[test]
fn recording_stop_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::EmptyEventPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated EmptyEventPayload schema");
    assert_schema_matches_file(generated, "recording-stop.schema.json", "recording-stop");
}

#[test]
fn overlay_hide_requested_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::EmptyEventPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated EmptyEventPayload schema");
    assert_schema_matches_file(
        generated,
        "overlay-hide-requested.schema.json",
        "overlay-hide-requested",
    );
}

#[test]
fn history_changed_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::EmptyEventPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated EmptyEventPayload schema");
    assert_schema_matches_file(generated, "history-changed.schema.json", "history-changed");
}

#[test]
fn stats_changed_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::EmptyEventPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated EmptyEventPayload schema");
    assert_schema_matches_file(generated, "stats-changed.schema.json", "stats-changed");
}

#[test]
fn settings_changed_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::SettingsChangedPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated SettingsChangedPayload schema");
    assert_schema_matches_file(
        generated,
        "settings-changed.schema.json",
        "settings-changed",
    );
}

#[test]
fn connection_state_changed_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::ConnectionStateChangedPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated ConnectionStateChangedPayload schema");
    assert_schema_matches_file(
        generated,
        "connection-state-changed.schema.json",
        "connection-state-changed",
    );
}

#[test]
fn overlay_audio_level_payload_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::OverlayAudioLevelPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated OverlayAudioLevelPayload schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("overlay-audio-level-payload.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"OverlayAudioLevelPayload schema changed. Regenerate overlay-audio-level-payload.schema.json using the export_overlay_audio_level_payload_schema bin.",
	);
}

#[test]
fn quick_ask_started_payload_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::QuickAskStartedPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated QuickAskStartedPayload schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("quick-ask-started-payload.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"QuickAskStartedPayload schema changed. Regenerate quick-ask-started-payload.schema.json using the export_quick_ask_started_payload_schema bin.",
	);
}

#[test]
fn quick_ask_answer_payload_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::QuickAskAnswerPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated QuickAskAnswerPayload schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("quick-ask-answer-payload.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"QuickAskAnswerPayload schema changed. Regenerate quick-ask-answer-payload.schema.json using the export_quick_ask_answer_payload_schema bin.",
	);
}

#[test]
fn mic_test_audio_level_payload_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::MicTestAudioLevelPayload);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated MicTestAudioLevelPayload schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("mic-test-audio-level-payload.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"MicTestAudioLevelPayload schema changed. Regenerate mic-test-audio-level-payload.schema.json using the export_mic_test_audio_level_payload_schema bin.",
	);
}
