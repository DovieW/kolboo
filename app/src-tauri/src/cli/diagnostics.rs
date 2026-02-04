use serde_json::json;
use tauri::{AppHandle, Manager};
use tauri_plugin_cli::Matches;

use crate::cli::{output_format_from, CliError, CliOutputFormat, CommandResult};
use crate::pipeline::SharedPipeline;

pub(crate) fn handle_diagnostics(
    app: &AppHandle,
    matches: &Matches,
) -> Result<(CliOutputFormat, CommandResult<serde_json::Value>), CliError> {
    let output_format = output_format_from(matches);

    let app_version = app.package_info().version.to_string();
    let os = std::env::consts::OS.to_string();
    let pipeline_state =
        crate::commands::recording::pipeline_get_state(app.state::<SharedPipeline>())
            .map_err(|err| CliError::Runtime(err.to_string()))?;
    let last_recording_diagnostics =
        crate::commands::recording::pipeline_get_last_recording_diagnostics(
            app.state::<SharedPipeline>(),
        )
        .map_err(|err| CliError::Runtime(err.to_string()))?;
    let audio_mute_supported = crate::audio_mute::is_supported();

    let payload = json!({
        "app_version": app_version,
        "os": os,
        "pipeline_state": pipeline_state,
        "audio_mute_supported": audio_mute_supported,
        "last_recording_diagnostics": last_recording_diagnostics,
    });

    Ok((
        output_format,
        CommandResult::success(Some(payload), Some("Diagnostics collected".to_string())),
    ))
}
