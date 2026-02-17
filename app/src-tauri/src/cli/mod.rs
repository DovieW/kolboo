mod config_export;
mod diagnostics;
mod errors;
mod logs;
mod output;
mod pipeline;
mod profiles;
mod settings;
mod types;
mod wav_info;

pub(crate) use errors::*;
pub(crate) use output::*;
pub(crate) use types::*;

/// Returns true if the current process appears to be invoked in "CLI mode".
///
/// We use this to avoid installing desktop-only, global, or single-instance behavior
/// when a user is running a CLI subcommand (e.g. `kolboo pipeline transcribe ...`).
///
/// Keep this intentionally conservative: only treat known subcommands and common
/// help/version flags as CLI invocations.
pub(crate) fn is_cli_invocation() -> bool {
    is_cli_invocation_from_args(std::env::args().skip(1))
}

fn is_cli_invocation_from_args<I>(mut args: I) -> bool
where
    I: Iterator<Item = String>,
{
    let Some(first) = args.next() else {
        return false;
    };
    let first = first.trim();
    if first.is_empty() {
        return false;
    }

    // Common flags that indicate a CLI run.
    if matches!(first, "-h" | "--help" | "help" | "-V" | "--version") {
        return true;
    }

    // Known top-level subcommands.
    matches!(
        first,
        "pipeline" | "settings" | "profiles" | "diagnostics" | "config" | "logs"
    )
}

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
        "logs" => logs::handle_logs(app, &subcommand.matches)?,
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

#[cfg(test)]
mod tests {
    use super::is_cli_invocation_from_args;

    #[test]
    fn detects_known_subcommands() {
        assert!(is_cli_invocation_from_args(
            ["pipeline".to_string()].into_iter()
        ));
        assert!(is_cli_invocation_from_args(
            ["settings".to_string()].into_iter()
        ));
        assert!(is_cli_invocation_from_args(
            ["profiles".to_string()].into_iter()
        ));
        assert!(is_cli_invocation_from_args(
            ["diagnostics".to_string()].into_iter()
        ));
        assert!(is_cli_invocation_from_args(
            ["config".to_string()].into_iter()
        ));
        assert!(is_cli_invocation_from_args(
            ["logs".to_string()].into_iter()
        ));
    }

    #[test]
    fn detects_help_and_version_flags() {
        assert!(is_cli_invocation_from_args(
            ["--help".to_string()].into_iter()
        ));
        assert!(is_cli_invocation_from_args(["-h".to_string()].into_iter()));
        assert!(is_cli_invocation_from_args(
            ["--version".to_string()].into_iter()
        ));
        assert!(is_cli_invocation_from_args(["-V".to_string()].into_iter()));
    }

    #[test]
    fn does_not_trigger_for_empty_or_unknown() {
        assert!(!is_cli_invocation_from_args([].into_iter()));
        assert!(!is_cli_invocation_from_args(["".to_string()].into_iter()));
        assert!(!is_cli_invocation_from_args(
            ["--some-flag".to_string()].into_iter()
        ));
        assert!(!is_cli_invocation_from_args(
            ["open".to_string()].into_iter()
        ));
    }
}
