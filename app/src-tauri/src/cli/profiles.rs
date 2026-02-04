use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri_plugin_cli::Matches;

use crate::cli::{arg_string, output_format_from, CliError, CliOutputFormat, CommandResult};
use crate::pipeline::SharedPipeline;
use crate::settings::store::{get_settings_store_or_err, SettingsReadMode};
use crate::settings::RewriteProgramPromptProfile;

#[derive(Debug, Serialize)]
struct ProfileSummary {
    id: String,
    name: String,
    disabled: bool,
}

#[derive(Debug, Serialize)]
struct ProfileListPayload {
    profiles: Vec<ProfileSummary>,
}

pub(crate) fn handle_profiles(
    app: &AppHandle,
    matches: &Matches,
) -> Result<(CliOutputFormat, CommandResult<Value>), CliError> {
    let Some(subcommand) = matches.subcommand.as_ref() else {
        return Err(CliError::Validation(
            "Missing profiles subcommand".to_string(),
        ));
    };

    match subcommand.name.as_str() {
        "list" => {
            let output_format = output_format_from(&subcommand.matches);
            let profiles = load_profiles(app)?;
            let payload = ProfileListPayload {
                profiles: profiles
                    .into_iter()
                    .map(|p| ProfileSummary {
                        id: p.id,
                        name: p.name,
                        disabled: p.disabled,
                    })
                    .collect(),
            };

            let payload_value = serde_json::to_value(payload).unwrap_or(Value::Null);
            Ok((
                output_format,
                CommandResult::success(
                    Some(serde_json::json!({ "kind": "list", "data": payload_value })),
                    None,
                ),
            ))
        }
        "use" => {
            let output_format = output_format_from(&subcommand.matches);
            let profile_id = arg_string(&subcommand.matches, "profile")
                .ok_or_else(|| CliError::Validation("Missing --profile".to_string()))?;

            let profiles = load_profiles(app)?;
            let selected = profiles
                .iter()
                .find(|p| p.id == profile_id)
                .ok_or_else(|| {
                    CliError::Validation(format!("Unknown profile id: {}", profile_id))
                })?;

            if selected.disabled {
                return Err(CliError::Validation(format!(
                    "Profile is disabled: {}",
                    profile_id
                )));
            }

            crate::commands::recording::pipeline_set_session_preset_lock(
                app.state::<SharedPipeline>(),
                Some(profile_id.clone()),
                None,
            )
            .map_err(|err| CliError::Runtime(err.to_string()))?;

            let selected_payload = ProfileSummary {
                id: profile_id,
                name: selected.name.clone(),
                disabled: selected.disabled,
            };
            let selected_value = serde_json::to_value(selected_payload).unwrap_or(Value::Null);
            let result = CommandResult::success(
                Some(serde_json::json!({ "kind": "selected", "data": selected_value })),
                Some("Profile selected for next session".to_string()),
            );

            Ok((output_format, result))
        }
        _ => Err(CliError::Validation(format!(
            "Unknown profiles subcommand: {}",
            subcommand.name
        ))),
    }
}

fn load_profiles(app: &AppHandle) -> Result<Vec<RewriteProgramPromptProfile>, CliError> {
    let store =
        get_settings_store_or_err(app, SettingsReadMode::Fresh).map_err(CliError::Runtime)?;

    let raw = store.get("rewrite_program_prompt_profiles");
    let profiles = raw
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();

    Ok(profiles)
}
