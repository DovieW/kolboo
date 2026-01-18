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
fn system_proxy_info_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::network::SystemProxyInfo);
    let generated: Value =
        serde_json::to_value(schema).expect("Failed to serialize generated SystemProxyInfo schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("system-proxy-info.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"SystemProxyInfo schema changed. Regenerate system-proxy-info.schema.json using the export_system_proxy_info_schema bin.",
	);
}

#[test]
fn windows_internet_proxy_settings_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::network::WindowsInternetProxySettings);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated WindowsInternetProxySettings schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("windows-internet-proxy-settings.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"WindowsInternetProxySettings schema changed. Regenerate windows-internet-proxy-settings.schema.json using the export_windows_internet_proxy_settings_schema bin.",
	);
}

#[test]
fn audio_settings_test_wavs_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::recording::AudioSettingsTestWavs);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated AudioSettingsTestWavs schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("audio-settings-test-wavs.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"AudioSettingsTestWavs schema changed. Regenerate audio-settings-test-wavs.schema.json using the export_audio_settings_test_wavs_schema bin.",
	);
}

#[test]
fn whisper_model_info_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::whisper::WhisperModelInfo);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated WhisperModelInfo schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("whisper-model-info.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"WhisperModelInfo schema changed. Regenerate whisper-model-info.schema.json using the export_whisper_model_info_schema bin.",
	);
}

#[test]
fn whisper_model_download_status_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::whisper::WhisperModelDownloadStatus);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated WhisperModelDownloadStatus schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("whisper-model-download-status.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"WhisperModelDownloadStatus schema changed. Regenerate whisper-model-download-status.schema.json using the export_whisper_model_download_status_schema bin.",
	);
}

#[test]
fn whisper_model_download_progress_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::whisper::WhisperModelDownloadProgress);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated WhisperModelDownloadProgress schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("whisper-model-download-progress.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"WhisperModelDownloadProgress schema changed. Regenerate whisper-model-download-progress.schema.json using the export_whisper_model_download_progress_schema bin.",
	);
}

#[test]
fn local_whisper_backend_status_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::whisper::LocalWhisperBackendStatusResponse);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated LocalWhisperBackendStatus schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("local-whisper-backend-status.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"LocalWhisperBackendStatus schema changed. Regenerate local-whisper-backend-status.schema.json using the export_local_whisper_backend_status_schema bin.",
	);
}

#[test]
fn local_whisper_model_load_event_schema_matches_checked_in_file() {
    let schema = schema_for!(crate::commands::whisper::LocalWhisperModelLoadEvent);
    let generated: Value = serde_json::to_value(schema)
        .expect("Failed to serialize generated LocalWhisperModelLoadEvent schema");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    path.push("local-whisper-model-load-event.schema.json");

    let checked_in = read_schema(&path);
    assert_eq!(
		generated, checked_in,
		"LocalWhisperModelLoadEvent schema changed. Regenerate local-whisper-model-load-event.schema.json using the export_local_whisper_model_load_event_schema bin.",
	);
}
