use serde_json::{json, Map, Value};
use tauri::AppHandle;
use tauri_plugin_cli::Matches;

use crate::cli::{arg_string, output_format_from, CliError, CliOutputFormat, CommandResult};
use crate::settings::store::{get_settings_store_or_err, SettingsReadMode};

pub(crate) fn handle_settings(
    app: &AppHandle,
    matches: &Matches,
) -> Result<(CliOutputFormat, CommandResult<Value>), CliError> {
    let Some(subcommand) = matches.subcommand.as_ref() else {
        return Err(CliError::Validation(
            "Missing settings subcommand".to_string(),
        ));
    };

    match subcommand.name.as_str() {
        "get" => {
            let output_format = output_format_from(&subcommand.matches);
            let key = arg_string(&subcommand.matches, "key")
                .ok_or_else(|| CliError::Validation("Missing --key".to_string()))?;
            let store = get_settings_store_or_err(app, SettingsReadMode::Fresh)
                .map_err(CliError::Runtime)?;

            let value = store
                .get(key.as_str())
                .ok_or_else(|| CliError::Validation(format!("Unknown setting key: {}", key)))?;

            Ok((
                output_format,
                CommandResult::success(Some(json!({ "key": key, "value": value })), None),
            ))
        }
        "set" => {
            let output_format = output_format_from(&subcommand.matches);
            let key = arg_string(&subcommand.matches, "key")
                .ok_or_else(|| CliError::Validation("Missing --key".to_string()))?;
            let raw_value = arg_string(&subcommand.matches, "value")
                .ok_or_else(|| CliError::Validation("Missing --value".to_string()))?;
            let value = parse_setting_value(raw_value.as_str());

            let mut patch = Map::new();
            patch.insert(key.clone(), value.clone());

            tauri::async_runtime::block_on(crate::commands::settings::settings_apply_patch(
                app.clone(),
                patch,
                Vec::new(),
            ))
            .map_err(|err| CliError::Runtime(err.to_string()))?;

            crate::commands::config::sync_pipeline_config(app.clone())
                .map_err(|err| CliError::Runtime(err.to_string()))?;

            Ok((
                output_format,
                CommandResult::success(
                    Some(json!({ "key": key, "value": value })),
                    Some("Setting updated".to_string()),
                ),
            ))
        }
        _ => Err(CliError::Validation(format!(
            "Unknown settings subcommand: {}",
            subcommand.name
        ))),
    }
}

fn parse_setting_value(raw: &str) -> Value {
    match serde_json::from_str::<Value>(raw) {
        Ok(value) => value,
        Err(_) => Value::String(raw.to_string()),
    }
}
