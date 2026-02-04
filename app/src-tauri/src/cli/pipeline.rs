use serde_json::json;
use tauri::{AppHandle, Manager};
use tauri_plugin_cli::Matches;

use crate::cli::{arg_string, output_format_from, CliError, CliOutputFormat, CommandResult};
use crate::pipeline::SharedPipeline;

pub(crate) fn handle_pipeline(
    app: &AppHandle,
    matches: &Matches,
) -> Result<(CliOutputFormat, CommandResult<serde_json::Value>), CliError> {
    let Some(subcommand) = matches.subcommand.as_ref() else {
        return Err(CliError::Validation(
            "Missing pipeline subcommand".to_string(),
        ));
    };

    match subcommand.name.as_str() {
        "run" => {
            let output_format = output_format_from(&subcommand.matches);
            let profile_id = arg_string(&subcommand.matches, "profile");
            let pipeline = app.state::<SharedPipeline>();

            if let Some(profile_id) = profile_id.as_deref() {
                pipeline
                    .set_session_profile_override(Some(profile_id.to_string()))
                    .map_err(|err| CliError::Runtime(err.to_string()))?;
            }

            crate::commands::recording::pipeline_start_recording(app.clone(), pipeline)
                .map_err(|err| CliError::Runtime(err.to_string()))?;

            let state =
                crate::commands::recording::pipeline_get_state(app.state::<SharedPipeline>())
                    .map_err(|err| CliError::Runtime(err.to_string()))?;

            Ok((
                output_format,
                CommandResult::success(
                    Some(json!({ "state": state })),
                    Some("Recording started".to_string()),
                ),
            ))
        }
        "stop" => {
            let output_format = output_format_from(&subcommand.matches);
            let pipeline = app.state::<SharedPipeline>();

            let transcript = tauri::async_runtime::block_on(
                crate::commands::recording::pipeline_stop_and_transcribe(app.clone(), pipeline),
            )
            .map_err(|err| CliError::Runtime(err.to_string()))?;

            Ok((
                output_format,
                CommandResult::success(
                    Some(json!({ "transcript": transcript })),
                    Some("Recording stopped".to_string()),
                ),
            ))
        }
        "transcribe" => {
            let output_format = output_format_from(&subcommand.matches);
            let file_path = arg_string(&subcommand.matches, "file")
                .ok_or_else(|| CliError::Validation("Missing --file".to_string()))?;
            let profile_id = arg_string(&subcommand.matches, "profile");

            let wav_bytes = std::fs::read(&file_path)
                .map_err(|err| CliError::Runtime(format!("Failed to read file: {err}")))?;
            if wav_bytes.is_empty() {
                return Err(CliError::Validation("Audio file is empty".to_string()));
            }

            let pipeline = app.state::<SharedPipeline>();
            let result = tauri::async_runtime::block_on(
                pipeline
                    .transcribe_wav_bytes_detailed_for_profile(wav_bytes, profile_id.as_deref()),
            )
            .map_err(|err| CliError::Runtime(err.to_string()))?;

            let payload = json!({
                "final_text": result.final_text,
                "stt_text": result.stt_text,
                "stt_duration_ms": result.stt_duration_ms,
                "llm_duration_ms": result.llm_duration_ms,
                "llm_provider_used": result.llm_provider_used,
                "llm_model_used": result.llm_model_used,
                "llm_outcome": result.llm_outcome.code(),
            });

            Ok((
                output_format,
                CommandResult::success(Some(payload), Some("Transcription complete".to_string())),
            ))
        }
        "status" => {
            let output_format = output_format_from(&subcommand.matches);
            let state =
                crate::commands::recording::pipeline_get_state(app.state::<SharedPipeline>())
                    .map_err(|err| CliError::Runtime(err.to_string()))?;

            Ok((
                output_format,
                CommandResult::success(
                    Some(json!({ "state": state })),
                    Some(format!("Pipeline state: {}", state)),
                ),
            ))
        }
        _ => Err(CliError::Validation(format!(
            "Unknown pipeline subcommand: {}",
            subcommand.name
        ))),
    }
}
