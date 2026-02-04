use serde_json::json;
use tauri::{AppHandle, Manager};
use tauri_plugin_cli::Matches;

use crate::cli::{output_format_from, CliError, CliOutputFormat, CommandResult};
use crate::pipeline::SharedPipeline;
use crate::settings::store::{get_settings_store_or_err, SettingsReadMode};

pub(crate) fn handle_config(
    app: &AppHandle,
    matches: &Matches,
) -> Result<(CliOutputFormat, CommandResult<serde_json::Value>), CliError> {
    let Some(subcommand) = matches.subcommand.as_ref() else {
        return Err(CliError::Validation(
            "Missing config subcommand".to_string(),
        ));
    };

    match subcommand.name.as_str() {
        "export" => {
            let output_format = output_format_from(&subcommand.matches);
            let store = get_settings_store_or_err(app, SettingsReadMode::Fresh)
                .map_err(CliError::Runtime)?;
            let mut settings = store.entries().clone();
            settings.retain(|(key, _)| !is_sensitive_key(key));

            let session_profile_lock =
                crate::commands::recording::pipeline_get_session_preset_lock(
                    app.state::<SharedPipeline>(),
                )
                .map_err(|err| CliError::Runtime(err.to_string()))?;

            let payload = json!({
                "generated_at": chrono::Utc::now().to_rfc3339(),
                "settings": settings,
                "session_profile_lock": session_profile_lock,
            });

            Ok((
                output_format,
                CommandResult::success(Some(payload), Some("Config exported".to_string())),
            ))
        }
        _ => Err(CliError::Validation(format!(
            "Unknown config subcommand: {}",
            subcommand.name
        ))),
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_lowercase();
    lower.contains("api_key") || lower.contains("token") || lower.contains("secret")
}
