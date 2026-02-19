use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::commands::{CommandError, CommandResult};
use crate::events;
use crate::licensing::{
    apply_refresh_failure, apply_refresh_success, build_login_state, build_transition,
    clear_session_material, load_session_material, normalize_license_state,
    persist_session_material, telemetry_context_for_state, LicenseState, LoginRequest,
    SessionMaterial,
};

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

const LICENSE_STATE_KEY: &str = "license_state";

#[derive(Debug, Deserialize)]
struct SupabaseAuthUser {
    id: String,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SupabasePasswordAuthResponse {
    access_token: String,
    refresh_token: String,
    user: SupabaseAuthUser,
}

#[derive(Debug, Deserialize)]
struct SupabaseRefreshAuthResponse {
    access_token: String,
    refresh_token: Option<String>,
    user: Option<SupabaseAuthUser>,
}

fn supabase_auth_config() -> Result<(String, String), CommandError> {
    let supabase_url = crate::commands::config::read_first_non_empty_env(&["TAURI_SUPABASE_URL"])
        .ok_or_else(|| {
        CommandError::new("Supabase auth is not configured", "auth")
            .with_code("auth_not_configured")
    })?;

    let publishable_key =
        crate::commands::config::read_first_non_empty_env(&["TAURI_SUPABASE_PUBLISHABLE_KEY"])
            .ok_or_else(|| {
                CommandError::new("Supabase publishable key is not configured", "auth")
                    .with_code("auth_not_configured")
            })?;

    Ok((
        supabase_url.trim_end_matches('/').to_string(),
        publishable_key,
    ))
}

async fn sign_in_with_supabase_password(
    email: &str,
    password: &str,
) -> CommandResult<SupabasePasswordAuthResponse> {
    let (supabase_url, publishable_key) = supabase_auth_config()?;
    let url = format!("{supabase_url}/auth/v1/token?grant_type=password");

    let response = crate::network::build_plain_http_client_with_user_agent("kolboo-auth")
        .post(url)
        .header("apikey", publishable_key)
        .header("content-type", "application/json")
        .json(&json!({
            "email": email,
            "password": password,
        }))
        .send()
        .await
        .map_err(|e| {
            CommandError::new(format!("Sign in request failed: {e}"), "auth")
                .with_code("auth_sign_in_failed")
        })?;

    if !response.status().is_success() {
        let (status, text) = crate::http::status_and_text(response).await;
        return Err(
            CommandError::new(format!("Sign in failed ({status}): {text}"), "auth")
                .with_code("auth_sign_in_failed"),
        );
    }

    response
        .json::<SupabasePasswordAuthResponse>()
        .await
        .map_err(|e| {
            CommandError::new(format!("Failed to parse sign in response: {e}"), "auth")
                .with_code("auth_response_parse_failed")
        })
}

async fn refresh_supabase_session(
    session: &SessionMaterial,
) -> CommandResult<SupabaseRefreshAuthResponse> {
    let (supabase_url, publishable_key) = supabase_auth_config()?;
    let url = format!("{supabase_url}/auth/v1/token?grant_type=refresh_token");

    let response = crate::network::build_plain_http_client_with_user_agent("kolboo-auth")
        .post(url)
        .header("apikey", publishable_key)
        .header("content-type", "application/json")
        .json(&json!({
            "refresh_token": session.refresh_token,
        }))
        .send()
        .await
        .map_err(|e| {
            CommandError::new(format!("Session refresh request failed: {e}"), "auth")
                .with_code("auth_refresh_failed")
        })?;

    if !response.status().is_success() {
        let (status, text) = crate::http::status_and_text(response).await;
        return Err(CommandError::new(
            format!("Session refresh failed ({status}): {text}"),
            "auth",
        )
        .with_code("auth_refresh_failed"));
    }

    response
        .json::<SupabaseRefreshAuthResponse>()
        .await
        .map_err(|e| {
            CommandError::new(format!("Failed to parse refresh response: {e}"), "auth")
                .with_code("auth_response_parse_failed")
        })
}

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
    let request = request.unwrap_or(LoginRequest {
        provider_hint: None,
        email: None,
        password: None,
    });

    let email = request.email.unwrap_or_default().trim().to_string();
    let password = request.password.unwrap_or_default();

    if email.is_empty() || password.trim().is_empty() {
        return Err(CommandError::new("Email and password are required", "auth")
            .with_code("auth_credentials_required"));
    }

    let auth = sign_in_with_supabase_password(email.as_str(), password.as_str()).await?;

    persist_session_material(
        &app,
        &SessionMaterial {
            access_token: auth.access_token,
            refresh_token: auth.refresh_token,
        },
    )
    .map_err(|e| CommandError::new("Failed to persist session", "auth").with_code(e))?;

    let provider_hint = request.provider_hint.as_deref();
    let mut state = build_login_state(provider_hint, Utc::now());
    state.user_id = Some(auth.user.id);
    state.email = auth.user.email.or(Some(email));
    save_license_state(&app, &state, "login_success")?;

    if let Err(e) = crate::commands::config::sync_pipeline_config(app.clone()) {
        log::warn!(
            "Login updated session but failed to sync pipeline config: {}",
            e
        );
    }

    Ok(state)
}

#[tauri::command]
pub async fn license_logout(app: AppHandle) -> CommandResult<LicenseState> {
    clear_session_material(&app)
        .map_err(|e| CommandError::new("Failed to clear session", "auth").with_code(e))?;

    let state = LicenseState::signed_out(Utc::now());
    save_license_state(&app, &state, "logout")?;

    if let Err(e) = crate::commands::config::sync_pipeline_config(app.clone()) {
        log::warn!(
            "Logout cleared session but failed to sync pipeline config: {}",
            e
        );
    }

    Ok(state)
}

#[tauri::command]
pub async fn license_refresh_entitlement(
    app: AppHandle,
    simulate_failure: Option<bool>,
) -> CommandResult<LicenseState> {
    let Some(session) = load_session_material(&app) else {
        let state = LicenseState::signed_out(Utc::now());
        save_license_state(&app, &state, "session_missing")?;
        tracing::warn!(
            target: "licensing",
            context = %telemetry_context_for_state(&state),
            "license refresh skipped: missing secure session material"
        );
        return Ok(state);
    };

    let current = load_license_state(&app)?;
    let now = Utc::now();
    let failed = simulate_failure.unwrap_or(false);

    let next = if failed {
        apply_refresh_failure(current, now)
    } else {
        match refresh_supabase_session(&session).await {
            Ok(refresh) => {
                let refreshed_session = SessionMaterial {
                    access_token: refresh.access_token,
                    refresh_token: refresh
                        .refresh_token
                        .unwrap_or_else(|| session.refresh_token.clone()),
                };

                persist_session_material(&app, &refreshed_session).map_err(|e| {
                    CommandError::new("Failed to persist refreshed session", "auth").with_code(e)
                })?;

                let mut next = apply_refresh_success(current, now);
                if let Some(user) = refresh.user {
                    next.user_id = Some(user.id);
                    if let Some(email) = user.email {
                        next.email = Some(email);
                    }
                }

                if let Err(e) = crate::commands::config::sync_pipeline_config(app.clone()) {
                    log::warn!(
                        "License refresh updated session but failed to sync pipeline config: {}",
                        e
                    );
                }

                next
            }
            Err(err) => {
                log::warn!("License refresh token exchange failed: {}", err.message);
                apply_refresh_failure(current, now)
            }
        }
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
