use chrono::Utc;
use tauri::AppHandle;

use crate::commands::{CommandError, CommandResult};

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

#[cfg(desktop)]
fn load_policy_state(app: &AppHandle) -> CommandResult<crate::policy::PolicyState> {
    let store = app
        .store("settings.json")
        .map_err(|e| CommandError::unknown(format!("Failed to open settings store: {e}")))?;
    let raw = store.get("policy_state");
    Ok(crate::policy::policy_state_for_command(raw, Utc::now()))
}

#[cfg(desktop)]
#[tauri::command]
pub async fn policy_get_state(app: AppHandle) -> CommandResult<crate::policy::PolicyState> {
    load_policy_state(&app)
}

#[cfg(not(desktop))]
#[tauri::command]
pub async fn policy_get_state(_app: AppHandle) -> CommandResult<crate::policy::PolicyState> {
    Ok(crate::policy::PolicyState::default())
}

#[cfg(desktop)]
#[tauri::command]
pub async fn policy_export_diagnostics(
    app: AppHandle,
) -> CommandResult<crate::policy::PolicyDiagnosticExport> {
    let state = load_policy_state(&app)?;
    Ok(crate::policy::build_policy_diagnostic_export(
        state,
        Utc::now(),
        None,
    ))
}

#[cfg(not(desktop))]
#[tauri::command]
pub async fn policy_export_diagnostics(
    _app: AppHandle,
) -> CommandResult<crate::policy::PolicyDiagnosticExport> {
    Ok(crate::policy::build_policy_diagnostic_export(
        crate::policy::PolicyState::default(),
        Utc::now(),
        None,
    ))
}
