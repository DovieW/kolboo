use chrono::Utc;
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::commands::{CommandError, CommandResult};
use crate::events;
use crate::licensing::{
    apply_refresh_failure, apply_refresh_success, build_login_state, build_transition,
    clear_session_material, load_session_material, normalize_license_state,
    persist_session_material, telemetry_context_for_state, LicenseState, LoginRequest,
};

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

const LICENSE_STATE_KEY: &str = "license_state";
#[cfg(desktop)]
fn load_license_state(app: &AppHandle) -> CommandResult<LicenseState> {
    let store = app
        .store("settings.json")
        .map_err(|e| CommandError::unknown(format!("Failed to open settings store: {e}")))?;
    Ok(normalize_license_state(
        store.get(LICENSE_STATE_KEY),
        Utc::now(),
    ))
}

#[cfg(not(desktop))]
fn load_license_state(_app: &AppHandle) -> CommandResult<LicenseState> {
    Ok(LicenseState::signed_out(Utc::now()))
}

#[cfg(desktop)]
fn save_license_state(app: &AppHandle, state: &LicenseState, reason: &str) -> CommandResult<()> {
    let store = app
        .store("settings.json")
        .map_err(|e| CommandError::unknown(format!("Failed to open settings store: {e}")))?;

    let previous = normalize_license_state(store.get(LICENSE_STATE_KEY), Utc::now());

    store.set(
        LICENSE_STATE_KEY.to_string(),
        serde_json::to_value(state).map_err(|e| {
            CommandError::unknown(format!("Failed to serialize license state: {e}"))
        })?,
    );

    store
        .save()
        .map_err(|e| CommandError::unknown(format!("Failed to save settings store: {e}")))?;

    let mut payload = crate::SettingsChangedPayload::new();
    payload.insert("license_state_changed".to_string(), json!(true));
    let transition = build_transition(previous.status, state.status, Utc::now(), reason);
    payload.insert(
        "license_transition".to_string(),
        serde_json::to_value(transition).unwrap_or_else(|_| json!({})),
    );
    let _ = app.emit(events::EVENT_SETTINGS_CHANGED, payload);

    Ok(())
}

#[cfg(not(desktop))]
fn save_license_state(_app: &AppHandle, _state: &LicenseState, _reason: &str) -> CommandResult<()> {
    Ok(())
}

#[tauri::command]
pub async fn license_get_state(app: AppHandle) -> CommandResult<LicenseState> {
    let state = load_license_state(&app)?;
    Ok(state)
}

#[tauri::command]
pub async fn license_start_login(
    app: AppHandle,
    request: Option<LoginRequest>,
) -> CommandResult<LicenseState> {
    persist_session_material(&app)
        .map_err(|e| CommandError::new("Failed to persist session", "auth").with_code(e))?;

    let provider_hint = request.as_ref().and_then(|r| r.provider_hint.as_deref());
    let state = build_login_state(provider_hint, Utc::now());
    save_license_state(&app, &state, "login_success")?;
    Ok(state)
}

#[tauri::command]
pub async fn license_logout(app: AppHandle) -> CommandResult<LicenseState> {
    clear_session_material(&app)
        .map_err(|e| CommandError::new("Failed to clear session", "auth").with_code(e))?;

    let state = LicenseState::signed_out(Utc::now());
    save_license_state(&app, &state, "logout")?;
    Ok(state)
}

#[tauri::command]
pub async fn license_refresh_entitlement(
    app: AppHandle,
    simulate_failure: Option<bool>,
) -> CommandResult<LicenseState> {
    if load_session_material(&app).is_none() {
        let state = LicenseState::signed_out(Utc::now());
        save_license_state(&app, &state, "session_missing")?;
        tracing::warn!(
            target: "licensing",
            context = %telemetry_context_for_state(&state),
            "license refresh skipped: missing secure session material"
        );
        return Ok(state);
    }

    let current = load_license_state(&app)?;
    let now = Utc::now();
    let failed = simulate_failure.unwrap_or(false);

    let next = if failed {
        apply_refresh_failure(current, now)
    } else {
        apply_refresh_success(current, now)
    };

    let reason = if failed {
        "refresh_failed"
    } else {
        "refresh_success"
    };
    save_license_state(&app, &next, reason)?;
    Ok(next)
}

#[tauri::command]
pub async fn license_get_management_url(_app: AppHandle) -> CommandResult<String> {
    Ok("https://github.com/DovieW/kolboo".to_string())
}
