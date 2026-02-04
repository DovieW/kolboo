mod config_export;
mod diagnostics;
mod errors;
mod output;
mod pipeline;
mod profiles;
mod settings;
mod types;

pub(crate) use errors::*;
pub(crate) use output::*;
pub(crate) use types::*;

pub(crate) fn handle_cli(
    app: &tauri::AppHandle,
    matches: &tauri_plugin_cli::Matches,
) -> Result<Option<i32>, CliError> {
    let Some(subcommand) = matches.subcommand.as_ref() else {
        return Ok(None);
    };

    let (output_format, result) = match subcommand.name.as_str() {
        "pipeline" => pipeline::handle_pipeline(app, &subcommand.matches)?,
        "settings" => settings::handle_settings(app, &subcommand.matches)?,
        "profiles" => profiles::handle_profiles(app, &subcommand.matches)?,
        "diagnostics" => diagnostics::handle_diagnostics(app, &subcommand.matches)?,
        "config" => config_export::handle_config(app, &subcommand.matches)?,
        _ => {
            return Err(CliError::Validation(format!(
                "Unsupported CLI command: {}",
                subcommand.name
            )));
        }
    };

    match output_format {
        CliOutputFormat::Json => {
            write_json(&result).map_err(|err| CliError::Runtime(err.to_string()))?;
        }
        CliOutputFormat::Human => {
            let message = result
                .message
                .clone()
                .unwrap_or_else(|| "Command completed".to_string());
            write_human(&message);
        }
    }

    Ok(Some(result.code))
}
