use base64::Engine;
use chrono::Utc;
use reqwest::Url;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::timeout,
};
use uuid::Uuid;

use crate::commands::{CommandError, CommandResult};
use crate::events;
use crate::licensing::{
    apply_refresh_failure, apply_refresh_success, build_login_state, build_transition,
    clear_session_material, load_session_material, normalize_license_state,
    normalize_token_exchange_trigger_set, persist_session_material, telemetry_context_for_state,
    AuthReasonCode, LicenseAuthContext, LicenseState, LicenseStatus, LoginRequest, PolicyStatus,
    SessionExchangeRequest, SessionExchangeResponse, SessionMaterial, TokenExchangeDecision,
    TokenExchangeTriggerSet,
};

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

const LICENSE_STATE_KEY: &str = "license_state";
const TOKEN_EXCHANGE_TRIGGER_SET_KEY: &str = "token_exchange_trigger_set";
const LOOPBACK_CALLBACK_PATH: &str = "/auth/callback";
const LOOPBACK_AUTH_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Deserialize)]
struct SupabaseAuthUser {
    id: String,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SupabaseSessionAuthResponse {
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

async fn exchange_supabase_auth_code(
    auth_code: &str,
    code_verifier: &str,
) -> CommandResult<SupabaseSessionAuthResponse> {
    let (supabase_url, publishable_key) = supabase_auth_config()?;
    let url = format!("{supabase_url}/auth/v1/token?grant_type=pkce");

    let response = crate::network::build_plain_http_client_with_user_agent("kolboo-auth")
        .post(url)
        .header("apikey", publishable_key)
        .header("content-type", "application/json")
        .json(&json!({
            "auth_code": auth_code,
            "code_verifier": code_verifier,
        }))
        .send()
        .await
        .map_err(|e| {
            CommandError::new(
                format!("Browser sign-in token exchange failed: {e}"),
                "auth",
            )
            .with_code("auth_sign_in_failed")
        })?;

    if !response.status().is_success() {
        let (status, text) = crate::http::status_and_text(response).await;
        return Err(CommandError::new(
            format!("Browser sign-in failed during token exchange ({status}): {text}"),
            "auth",
        )
        .with_code("auth_sign_in_failed"));
    }

    response
        .json::<SupabaseSessionAuthResponse>()
        .await
        .map_err(|e| {
            CommandError::new(format!("Failed to parse sign in response: {e}"), "auth")
                .with_code("auth_response_parse_failed")
        })
}

async fn sign_in_supabase_with_password(
    email: &str,
    password: &str,
) -> CommandResult<SupabaseSessionAuthResponse> {
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
            CommandError::new(format!("Email sign-in request failed: {e}"), "auth")
                .with_code("auth_sign_in_failed")
        })?;

    if !response.status().is_success() {
        let (status, text) = crate::http::status_and_text(response).await;
        return Err(
            CommandError::new(format!("Email sign-in failed ({status}): {text}"), "auth")
                .with_code("auth_sign_in_failed"),
        );
    }

    response
        .json::<SupabaseSessionAuthResponse>()
        .await
        .map_err(|e| {
            CommandError::new(format!("Failed to parse sign in response: {e}"), "auth")
                .with_code("auth_response_parse_failed")
        })
}

fn resolve_password_login_credentials(
    request: &LoginRequest,
) -> CommandResult<Option<(String, String)>> {
    let email = request
        .email
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let password = request
        .password
        .as_deref()
        .filter(|value| !value.is_empty());

    match (email, password) {
        (Some(email), Some(password)) => Ok(Some((email.to_string(), password.to_string()))),
        (None, None) => Ok(None),
        _ => Err(CommandError::new(
            "Email and password are both required for email sign-in.",
            "auth",
        )
        .with_code("auth_credentials_missing")),
    }
}

fn resolve_supabase_auth_provider(requested: Option<&str>) -> Result<String, CommandError> {
    requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            crate::commands::config::read_first_non_empty_env(&[
                "TAURI_AUTH_PROVIDER",
                "TAURI_SUPABASE_AUTH_PROVIDER",
            ])
        })
        .ok_or_else(|| {
            CommandError::new(
                "Browser sign-in provider is not configured. Set TAURI_AUTH_PROVIDER.",
                "auth",
            )
            .with_code("auth_not_configured")
        })
}

fn generate_pkce_code_verifier() -> String {
    format!(
        "{}{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple(),
    )
}

fn generate_oauth_state() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn build_pkce_code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

fn build_loopback_redirect_uri(port: u16) -> String {
    format!("http://127.0.0.1:{port}{LOOPBACK_CALLBACK_PATH}")
}

fn build_supabase_authorize_url(
    supabase_url: &str,
    auth_provider: &str,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> CommandResult<Url> {
    let mut url = Url::parse(&format!("{supabase_url}/auth/v1/authorize")).map_err(|e| {
        CommandError::new(format!("Supabase auth URL is invalid: {e}"), "auth")
            .with_code("auth_not_configured")
    })?;

    url.query_pairs_mut()
        .append_pair("provider", auth_provider)
        .append_pair("redirect_to", redirect_uri)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "s256")
        .append_pair("state", state)
        .append_pair("scopes", "email profile openid");

    Ok(url)
}

fn parse_http_request_target(request: &str) -> Option<&str> {
    request.lines().next()?.split_whitespace().nth(1)
}

fn extract_auth_code_from_callback_target(
    target: &str,
    expected_state: &str,
) -> Result<String, String> {
    let callback_url = Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| "The browser callback URL was invalid.".to_string())?;

    if callback_url.path() != LOOPBACK_CALLBACK_PATH {
        return Err("The browser returned to an unexpected callback path.".to_string());
    }

    let mut actual_state: Option<String> = None;
    let mut auth_code: Option<String> = None;
    let mut auth_error: Option<String> = None;
    let mut auth_error_description: Option<String> = None;

    for (key, value) in callback_url.query_pairs() {
        match key.as_ref() {
            "state" => actual_state = Some(value.into_owned()),
            "code" => auth_code = Some(value.into_owned()),
            "error" => auth_error = Some(value.into_owned()),
            "error_description" => auth_error_description = Some(value.into_owned()),
            _ => {}
        }
    }

    if actual_state.as_deref() != Some(expected_state) {
        return Err("The browser sign-in state did not match the original request.".to_string());
    }

    if let Some(error) = auth_error {
        let description = auth_error_description
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(error);
        return Err(format!(
            "The identity provider returned an error: {description}"
        ));
    }

    auth_code
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "The browser callback did not include an authorization code.".to_string())
}

async fn write_loopback_response(
    stream: &mut tokio::net::TcpStream,
    status_line: &str,
    title: &str,
    message: &str,
) {
    let escaped_message = message
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title></head><body><h1>{title}</h1><p>{escaped_message}</p></body></html>"
    );
    let response = format!(
        "{status_line}\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.as_bytes().len()
    );

    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.flush().await;
}

async fn wait_for_supabase_auth_code(
    listener: TcpListener,
    expected_state: &str,
) -> CommandResult<String> {
    let (mut stream, _) = timeout(LOOPBACK_AUTH_TIMEOUT, listener.accept())
        .await
        .map_err(|_| {
            CommandError::new("Timed out waiting for browser sign-in to complete.", "auth")
                .with_code("auth_callback_timeout")
        })?
        .map_err(|e| {
            CommandError::new(format!("Failed to accept browser callback: {e}"), "auth")
                .with_code("auth_callback_failed")
        })?;

    let mut buffer = vec![0_u8; 8192];
    let bytes_read = timeout(LOOPBACK_AUTH_TIMEOUT, stream.read(&mut buffer))
        .await
        .map_err(|_| {
            CommandError::new("Timed out while reading the browser callback.", "auth")
                .with_code("auth_callback_timeout")
        })?
        .map_err(|e| {
            CommandError::new(format!("Failed to read browser callback: {e}"), "auth")
                .with_code("auth_callback_failed")
        })?;

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let result = parse_http_request_target(&request)
        .ok_or_else(|| "The browser callback was malformed.".to_string())
        .and_then(|target| extract_auth_code_from_callback_target(target, expected_state));

    match result {
        Ok(auth_code) => {
            write_loopback_response(
                &mut stream,
                "HTTP/1.1 200 OK",
                "Kolboo sign-in complete",
                "You can close this browser tab and return to Kolboo.",
            )
            .await;
            Ok(auth_code)
        }
        Err(message) => {
            write_loopback_response(
                &mut stream,
                "HTTP/1.1 400 Bad Request",
                "Kolboo sign-in failed",
                &message,
            )
            .await;
            Err(CommandError::new(message, "auth").with_code("auth_callback_failed"))
        }
    }
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

fn auth_entitlements_for_tier(tier: crate::licensing::LicenseTier) -> Vec<String> {
    match tier {
        crate::licensing::LicenseTier::Community => Vec::new(),
        crate::licensing::LicenseTier::Personal => vec!["managed_inference".to_string()],
        crate::licensing::LicenseTier::Enterprise => {
            vec![
                "managed_inference".to_string(),
                "enterprise_policy".to_string(),
            ]
        }
    }
}

fn build_auth_context(
    state: &LicenseState,
    has_secure_session: bool,
    issuer: Option<String>,
) -> LicenseAuthContext {
    let session_usable = has_secure_session
        && matches!(state.status, LicenseStatus::Active | LicenseStatus::Grace)
        && state
            .user_id
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);

    LicenseAuthContext {
        authenticated: session_usable,
        secure_session_present: has_secure_session,
        subject_id: state.user_id.clone(),
        issuer,
        mode: state.tier,
        org_id: state.org.as_ref().map(|org| org.org_id.clone()),
        entitlements: if session_usable {
            auth_entitlements_for_tier(state.tier)
        } else {
            Vec::new()
        },
        policy_status: if session_usable {
            PolicyStatus::Allow
        } else {
            PolicyStatus::Deny
        },
        reason_code: if session_usable {
            None
        } else if !has_secure_session || matches!(state.status, LicenseStatus::SignedOut) {
            Some(AuthReasonCode::ReauthRequired)
        } else if matches!(state.status, LicenseStatus::Expired) {
            Some(AuthReasonCode::TokenInvalid)
        } else {
            Some(AuthReasonCode::Unknown)
        },
    }
}

#[cfg(desktop)]
fn load_token_exchange_trigger_set(app: &AppHandle) -> CommandResult<TokenExchangeTriggerSet> {
    let store = app
        .store("settings.json")
        .map_err(|e| CommandError::unknown(format!("Failed to open settings store: {e}")))?;
    Ok(normalize_token_exchange_trigger_set(
        store.get(TOKEN_EXCHANGE_TRIGGER_SET_KEY),
        Utc::now(),
    ))
}

#[cfg(not(desktop))]
fn load_token_exchange_trigger_set(_app: &AppHandle) -> CommandResult<TokenExchangeTriggerSet> {
    Ok(normalize_token_exchange_trigger_set(None, Utc::now()))
}

fn build_session_exchange_placeholder_response(
    trigger_set: TokenExchangeTriggerSet,
) -> SessionExchangeResponse {
    let decision = trigger_set.decision;
    let reason = match decision {
        TokenExchangeDecision::DirectIdpToken => {
            "Token exchange remains in direct_idp_token mode until one or more enterprise triggers are active."
                .to_string()
        }
        TokenExchangeDecision::AdoptTokenExchange => {
            "Token exchange has been selected by the trigger set, but desktop session exchange is still a placeholder in this build."
                .to_string()
        }
    };

    SessionExchangeResponse {
        enabled: false,
        decision,
        trigger_set,
        session_token: None,
        refresh_token: None,
        expires_at: None,
        claims: json!({}),
        reason,
    }
}

#[tauri::command]
pub async fn license_get_state(app: AppHandle) -> CommandResult<LicenseState> {
    let state = load_license_state(&app)?;
    Ok(state)
}

#[tauri::command]
pub async fn license_get_auth_context(app: AppHandle) -> CommandResult<LicenseAuthContext> {
    let state = load_license_state(&app)?;
    let has_secure_session = load_session_material(&app).is_some();
    let issuer = crate::commands::config::read_first_non_empty_env(&[
        "TAURI_SUPABASE_URL",
        "TAURI_AUTH_ISSUER",
    ]);
    Ok(build_auth_context(&state, has_secure_session, issuer))
}

#[tauri::command]
pub async fn license_get_session_access_token(app: AppHandle) -> CommandResult<Option<String>> {
    Ok(load_session_material(&app).map(|session| session.access_token))
}

#[tauri::command]
pub async fn license_exchange_session(
    app: AppHandle,
    request: SessionExchangeRequest,
) -> CommandResult<SessionExchangeResponse> {
    if request.upstream_access_token.trim().is_empty() {
        return Err(CommandError::new(
            "An upstream access token is required for session exchange.",
            "auth",
        )
        .with_code("token_invalid"));
    }

    let trigger_set = load_token_exchange_trigger_set(&app)?;
    Ok(build_session_exchange_placeholder_response(trigger_set))
}

#[tauri::command]
pub async fn license_start_login(
    app: AppHandle,
    request: Option<LoginRequest>,
) -> CommandResult<LicenseState> {
    let request = request.unwrap_or(LoginRequest {
        provider_hint: None,
        auth_provider: None,
        email: None,
        password: None,
    });

    if let Some((email, password)) = resolve_password_login_credentials(&request)? {
        let auth = sign_in_supabase_with_password(email.as_str(), password.as_str()).await?;

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
        state.email = auth.user.email;
        save_license_state(&app, &state, "password_login_success")?;

        if let Err(e) = crate::commands::config::sync_pipeline_config(app.clone()) {
            log::warn!(
                "Login updated session but failed to sync pipeline config: {}",
                e
            );
        }

        return Ok(state);
    }

    let (supabase_url, _) = supabase_auth_config()?;
    let auth_provider = resolve_supabase_auth_provider(request.auth_provider.as_deref())?;
    let code_verifier = generate_pkce_code_verifier();
    let code_challenge = build_pkce_code_challenge(&code_verifier);
    let state_token = generate_oauth_state();
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| {
        CommandError::new(
            format!("Failed to start local browser callback server: {e}"),
            "auth",
        )
        .with_code("auth_callback_bind_failed")
    })?;
    let redirect_uri = build_loopback_redirect_uri(
        listener
            .local_addr()
            .map_err(|e| {
                CommandError::new(
                    format!("Failed to resolve local browser callback address: {e}"),
                    "auth",
                )
                .with_code("auth_callback_bind_failed")
            })?
            .port(),
    );
    let authorize_url = build_supabase_authorize_url(
        &supabase_url,
        &auth_provider,
        &redirect_uri,
        &state_token,
        &code_challenge,
    )?;

    open::that(authorize_url.as_str()).map_err(|e| {
        CommandError::new(format!("Failed to open browser for sign in: {e}"), "auth")
            .with_code("auth_browser_open_failed")
    })?;

    let auth_code = wait_for_supabase_auth_code(listener, &state_token).await?;
    let auth = exchange_supabase_auth_code(auth_code.as_str(), code_verifier.as_str()).await?;

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
    state.email = auth.user.email;
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

    if let Err(e) = crate::commands::policy::clear_cached_policy_state(&app) {
        log::warn!(
            "Logout cleared session but failed to clear cached policy state: {}",
            e
        );
    }

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

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};
    use reqwest::Url;

    use crate::licensing::{LicenseTier, PolicyStatus};

    use super::{
        build_auth_context, build_pkce_code_challenge, build_session_exchange_placeholder_response,
        build_supabase_authorize_url, extract_auth_code_from_callback_target,
        resolve_password_login_credentials,
    };

    #[test]
    fn auth_context_denies_when_signed_out() {
        let now = Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 0).unwrap();
        let state = crate::licensing::LicenseState::signed_out(now);

        let context = build_auth_context(&state, false, Some("https://issuer.test".to_string()));

        assert!(!context.authenticated);
        assert!(!context.secure_session_present);
        assert_eq!(context.policy_status, PolicyStatus::Deny);
        assert_eq!(
            context.reason_code,
            Some(crate::licensing::AuthReasonCode::ReauthRequired)
        );
    }

    #[test]
    fn auth_context_allows_for_active_session() {
        let now = Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 0).unwrap();
        let mut state = crate::licensing::LicenseState::signed_out(now);
        state.status = crate::licensing::LicenseStatus::Active;
        state.user_id = Some("user-123".to_string());
        state.tier = LicenseTier::Enterprise;
        state.expires_at = Some(now + Duration::days(30));

        let context = build_auth_context(&state, true, Some("https://issuer.test".to_string()));

        assert!(context.authenticated);
        assert!(context.secure_session_present);
        assert_eq!(context.policy_status, PolicyStatus::Allow);
        assert_eq!(context.reason_code, None);
        assert_eq!(context.entitlements.len(), 2);
    }

    #[test]
    fn auth_context_marks_expired_as_token_invalid() {
        let now = Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 0).unwrap();
        let mut state = crate::licensing::LicenseState::signed_out(now);
        state.status = crate::licensing::LicenseStatus::Expired;
        state.user_id = Some("user-123".to_string());

        let context = build_auth_context(&state, true, Some("https://issuer.test".to_string()));

        assert!(!context.authenticated);
        assert!(context.secure_session_present);
        assert_eq!(
            context.reason_code,
            Some(crate::licensing::AuthReasonCode::TokenInvalid)
        );
    }

    #[test]
    fn auth_context_allows_for_grace_when_secure_session_present() {
        let now = Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 0).unwrap();
        let mut state = crate::licensing::LicenseState::signed_out(now);
        state.status = crate::licensing::LicenseStatus::Grace;
        state.user_id = Some("user-123".to_string());
        state.tier = LicenseTier::Personal;

        let context = build_auth_context(&state, true, Some("https://issuer.test".to_string()));

        assert!(context.authenticated);
        assert!(context.secure_session_present);
        assert_eq!(context.policy_status, PolicyStatus::Allow);
        assert_eq!(context.reason_code, None);
        assert_eq!(context.entitlements, vec!["managed_inference".to_string()]);
    }

    #[test]
    fn auth_context_requires_reauth_when_secure_session_missing() {
        let now = Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 0).unwrap();
        let mut state = crate::licensing::LicenseState::signed_out(now);
        state.status = crate::licensing::LicenseStatus::Active;
        state.user_id = Some("user-123".to_string());
        state.tier = LicenseTier::Enterprise;

        let context = build_auth_context(&state, false, Some("https://issuer.test".to_string()));

        assert!(!context.authenticated);
        assert!(!context.secure_session_present);
        assert_eq!(context.policy_status, PolicyStatus::Deny);
        assert_eq!(
            context.reason_code,
            Some(crate::licensing::AuthReasonCode::ReauthRequired)
        );
        assert!(context.entitlements.is_empty());
    }

    #[test]
    fn pkce_code_challenge_matches_rfc_example() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

        let challenge = build_pkce_code_challenge(verifier);

        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn authorize_url_includes_pkce_parameters() {
        let url = build_supabase_authorize_url(
            "https://example.supabase.co",
            "google",
            "http://127.0.0.1:43123/auth/callback",
            "state-123",
            "challenge-456",
        )
        .expect("authorize url");
        let parsed = Url::parse(url.as_ref()).expect("parse authorize url");
        let params = parsed.query_pairs().into_owned().collect::<Vec<_>>();

        assert!(params.contains(&("provider".to_string(), "google".to_string())));
        assert!(params.contains(&(
            "redirect_to".to_string(),
            "http://127.0.0.1:43123/auth/callback".to_string(),
        )));
        assert!(params.contains(&("code_challenge".to_string(), "challenge-456".to_string(),)));
        assert!(params.contains(&("code_challenge_method".to_string(), "s256".to_string(),)));
        assert!(params.contains(&("state".to_string(), "state-123".to_string())));
    }

    #[test]
    fn password_login_credentials_trim_email_and_preserve_password() {
        let request = crate::licensing::LoginRequest {
            provider_hint: None,
            auth_provider: None,
            email: Some(" user@example.com ".to_string()),
            password: Some(" secret-with-space ".to_string()),
        };

        let credentials = resolve_password_login_credentials(&request)
            .expect("valid credentials")
            .expect("password login");

        assert_eq!(credentials.0, "user@example.com");
        assert_eq!(credentials.1, " secret-with-space ");
    }

    #[test]
    fn password_login_credentials_absent_allows_oauth_fallback() {
        let request = crate::licensing::LoginRequest {
            provider_hint: None,
            auth_provider: Some("google".to_string()),
            email: None,
            password: None,
        };

        let credentials =
            resolve_password_login_credentials(&request).expect("missing credentials are ok");

        assert_eq!(credentials, None);
    }

    #[test]
    fn password_login_credentials_reject_partial_input() {
        let request = crate::licensing::LoginRequest {
            provider_hint: None,
            auth_provider: None,
            email: Some("user@example.com".to_string()),
            password: None,
        };

        let error = resolve_password_login_credentials(&request)
            .expect_err("partial credentials should fail");

        assert_eq!(error.code.as_deref(), Some("auth_credentials_missing"));
    }

    #[test]
    fn callback_parser_extracts_auth_code_when_state_matches() {
        let code = extract_auth_code_from_callback_target(
            "/auth/callback?code=auth-code-123&state=expected-state",
            "expected-state",
        )
        .expect("auth code");

        assert_eq!(code, "auth-code-123");
    }

    #[test]
    fn callback_parser_rejects_state_mismatch() {
        let error = extract_auth_code_from_callback_target(
            "/auth/callback?code=auth-code-123&state=wrong-state",
            "expected-state",
        )
        .expect_err("state mismatch should fail");

        assert!(error.contains("state did not match"));
    }

    #[test]
    fn session_exchange_placeholder_reports_direct_mode_by_default() {
        let response = build_session_exchange_placeholder_response(
            crate::licensing::normalize_token_exchange_trigger_set(None, Utc::now()),
        );

        assert!(!response.enabled);
        assert_eq!(
            response.decision,
            crate::licensing::TokenExchangeDecision::DirectIdpToken
        );
        assert!(response.reason.contains("direct_idp_token"));
    }

    #[test]
    fn session_exchange_placeholder_reports_adoption_when_triggered() {
        let response = build_session_exchange_placeholder_response(
            crate::licensing::normalize_token_exchange_trigger_set(
                Some(serde_json::json!({
                    "embedded_claims_required": true,
                })),
                Utc::now(),
            ),
        );

        assert!(!response.enabled);
        assert_eq!(
            response.decision,
            crate::licensing::TokenExchangeDecision::AdoptTokenExchange
        );
        assert!(response.reason.contains("placeholder"));
    }
}
