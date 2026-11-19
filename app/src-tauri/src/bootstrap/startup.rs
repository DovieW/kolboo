//! Startup defaults, visibility, and optional account work. OS login registration
//! stays machine-local; upgrades must not turn an existing opt-out back on.
use tauri_plugin_autostart::ManagerExt;

pub(crate) fn initialize_start_at_login(app: &tauri::AppHandle, new_install: bool) {
    apply_login_default(new_install && !crate::cli::is_cli_invocation(), || {
        app.autolaunch().enable()
    });
}

fn apply_login_default<E>(new_install: bool, enable: impl FnOnce() -> Result<(), E>) {
    if new_install && enable().is_err() {
        // Registration failure is recoverable through Settings, not a reason
        // to prevent startup. Do not log OS paths or error payloads.
        log::warn!("Start-at-login default could not be enabled; retry in Settings → UI");
    }
}

pub(crate) fn should_show_window(guide_state: &str, show_on_launch: bool) -> bool {
    guide_state == "pending" || show_on_launch
}

/// Account refresh is optional startup work. A local/BYOK installation without
/// secure session material must not create a task or contact account services.
/// The returned lease lets native rehearsals await the real refresh policy.
pub(crate) fn spawn_startup_session_refresh(
    app: &tauri::AppHandle,
) -> Option<tauri::async_runtime::JoinHandle<()>> {
    crate::licensing::load_session_material(app)?;
    let app = app.clone();
    Some(tauri::async_runtime::spawn(async move {
        match crate::commands::licensing::license_refresh_entitlement(app, Some(false)).await {
            Ok(state) => log::info!(
                "Startup silent entitlement refresh succeeded (status={:?})",
                state.status
            ),
            // The command owns account failure classification and recovery.
            // Its message can contain upstream/account context; never repeat it.
            Err(_) => log::warn!("Startup silent entitlement refresh failed; retry in Account"),
        }
    }))
}

#[cfg(test)]
mod tests;
