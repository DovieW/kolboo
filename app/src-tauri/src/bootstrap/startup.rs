//! Startup defaults and visibility policy. OS login registration stays
//! machine-local; upgrades must not turn an existing opt-out back on.
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

#[cfg(test)]
mod tests;
