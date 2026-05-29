use base64::Engine;
use chrono::Utc;
use reqwest::Method;
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
    apply_refresh_failure, build_login_state, build_transition, clear_session_material,
    load_session_material, normalize_license_state, normalize_token_exchange_trigger_set,
    persist_session_material, telemetry_context_for_state, AuthReasonCode, LicenseAuthContext,
    LicenseState, LicenseStatus, LicenseTier, LoginRequest, PolicyStatus, SessionExchangeRequest,
    SessionExchangeResponse, SessionMaterial, SignupRequest, SignupResponse, TokenExchangeDecision,
    TokenExchangeTriggerSet,
};

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

const LICENSE_STATE_KEY: &str = "license_state";
const TOKEN_EXCHANGE_TRIGGER_SET_KEY: &str = "token_exchange_trigger_set";
const LOOPBACK_CALLBACK_PATH: &str = "/auth/callback";
const LOOPBACK_AUTH_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Deserialize)]
struct SupabaseSignupAuthResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    user: Option<SupabaseAuthUser>,
    id: Option<String>,
    email: Option<String>,
}

impl SupabaseSignupAuthResponse {
    fn display_email(&self) -> Option<String> {
        self.user
            .as_ref()
            .and_then(|user| user.email.clone())
            .or_else(|| self.email.clone())
    }

    fn to_session_auth_response(&self) -> Option<SupabaseSessionAuthResponse> {
        let user = self.user.clone().or_else(|| {
            self.id.as_ref().map(|id| SupabaseAuthUser {
                id: id.clone(),
                email: self.email.clone(),
            })
        })?;

        Some(SupabaseSessionAuthResponse {
            access_token: self.access_token.clone()?,
            refresh_token: self.refresh_token.clone()?,
            user,
        })
    }
}

#[derive(Debug, Deserialize)]
struct LicensePortalUrlResponse {
    available: bool,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PublicAuthSessionCallbackRequest {
    state: String,
    access_token: String,
    refresh_token: String,
    user_id: String,
    email: Option<String>,
}

#[derive(Debug)]
struct PublicAuthSessionCallback {
    session: SessionMaterial,
    user_id: String,
    email: Option<String>,
}

enum BrowserAuthCallback {
    AuthorizationCode(String),
    Session(PublicAuthSessionCallback),
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

fn api_base_url() -> Option<String> {
    crate::commands::config::read_first_non_empty_env(&["TAURI_API_BASE_URL"])
        .map(|value| value.trim_end_matches('/').to_string())
}

fn public_auth_page_url() -> Option<String> {
    crate::commands::config::read_first_non_empty_env(&[
        "TAURI_PUBLIC_AUTH_PAGE_URL",
        "TAURI_AUTH_PAGE_URL",
    ])
    .map(|value| value.trim().to_string())
}

fn license_api_client() -> reqwest::Client {
    crate::network::build_plain_http_client_with_user_agent("kolboo-auth")
}

fn license_api_url(base_url: &str, path: &str) -> String {
    format!("{base_url}{path}")
}

fn merge_hydrated_identity(
    mut state: LicenseState,
    user_id: Option<String>,
    email: Option<String>,
) -> LicenseState {
    if state.user_id.is_none() {
        state.user_id = user_id;
    }
    if state.email.is_none() {
        state.email = email;
    }
    state
}

fn build_signed_in_fallback_state(
    user_id: String,
    email: Option<String>,
    now: chrono::DateTime<Utc>,
) -> LicenseState {
    let mut state = build_login_state(None, now);
    state.user_id = Some(user_id);
    state.email = email;
    state
}

async fn fetch_license_state_from_api(
    access_token: &str,
    method: Method,
    path: &str,
) -> CommandResult<LicenseState> {
    let base_url = api_base_url().ok_or_else(|| {
        CommandError::new("Desktop license API is not configured", "auth")
            .with_code("license_not_configured")
    })?;
    let url = license_api_url(&base_url, path);
    let request = match method {
        Method::GET => license_api_client().get(&url),
        Method::POST => license_api_client().post(&url),
        _ => unreachable!("license state helper only supports GET/POST"),
    };

    let response = crate::http::with_cloudflare_access_headers_if_target(
        request.bearer_auth(access_token),
        &url,
    )
    .send()
    .await
    .map_err(|e| {
        CommandError::new(format!("License hydration request failed: {e}"), "auth")
            .with_code("license_state_unavailable")
    })?;

    if !response.status().is_success() {
        let (status, text) = crate::http::status_and_text(response).await;
        return Err(CommandError::new(
            format!("License hydration failed ({status}): {text}"),
            "auth",
        )
        .with_code("license_state_unavailable"));
    }

    let raw = response.json::<serde_json::Value>().await.map_err(|e| {
        CommandError::new(
            format!("Failed to parse license hydration response: {e}"),
            "auth",
        )
        .with_code("license_state_parse_failed")
    })?;

    Ok(normalize_license_state(Some(raw), Utc::now()))
}

async fn fetch_license_portal_url_from_api(
    access_token: &str,
) -> CommandResult<LicensePortalUrlResponse> {
    let base_url = api_base_url().ok_or_else(|| {
        CommandError::new("Desktop license API is not configured", "auth")
            .with_code("license_not_configured")
    })?;
    let url = license_api_url(&base_url, "/v1/license/portal-url");

    let response = crate::http::with_cloudflare_access_headers_if_target(
        license_api_client().get(&url).bearer_auth(access_token),
        &url,
    )
    .send()
    .await
    .map_err(|e| {
        CommandError::new(format!("Billing portal lookup failed: {e}"), "auth")
            .with_code("portal_unavailable")
    })?;

    if !response.status().is_success() {
        let (status, text) = crate::http::status_and_text(response).await;
        return Err(CommandError::new(
            format!("Billing portal lookup failed ({status}): {text}"),
            "auth",
        )
        .with_code("portal_unavailable"));
    }

    response
        .json::<LicensePortalUrlResponse>()
        .await
        .map_err(|e| {
            CommandError::new(
                format!("Failed to parse billing portal response: {e}"),
                "auth",
            )
            .with_code("portal_unavailable")
        })
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

async fn sign_up_supabase_with_password(
    email: &str,
    password: &str,
) -> CommandResult<SupabaseSignupAuthResponse> {
    let (supabase_url, publishable_key) = supabase_auth_config()?;
    let url = format!("{supabase_url}/auth/v1/signup");

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
            CommandError::new(format!("Email sign-up request failed: {e}"), "auth")
                .with_code("auth_sign_up_failed")
        })?;

    if !response.status().is_success() {
        let (status, text) = crate::http::status_and_text(response).await;
        return Err(
            CommandError::new(format!("Email sign-up failed ({status}): {text}"), "auth")
                .with_code("auth_sign_up_failed"),
        );
    }

    response
        .json::<SupabaseSignupAuthResponse>()
        .await
        .map_err(|e| {
            CommandError::new(format!("Failed to parse sign-up response: {e}"), "auth")
                .with_code("auth_response_parse_failed")
        })
}

async fn persist_session_material_and_hydrate_license_state(
    app: &AppHandle,
    session_material: SessionMaterial,
    user_id: String,
    user_email: Option<String>,
    hydration_method: Method,
    hydration_path: &str,
    save_reason: &str,
    hydration_failure_context: &str,
) -> CommandResult<LicenseState> {
    persist_session_material(app, &session_material)
        .map_err(|e| CommandError::new("Failed to persist session", "auth").with_code(e))?;

    let fallback_state = build_signed_in_fallback_state(user_id, user_email, Utc::now());
    let state = match fetch_license_state_from_api(
        &session_material.access_token,
        hydration_method,
        hydration_path,
    )
    .await
    {
        Ok(hydrated) => merge_hydrated_identity(
            hydrated,
            fallback_state.user_id.clone(),
            fallback_state.email.clone(),
        ),
        Err(error) => {
            log::warn!(
                "{} completed, but api-edge license hydration failed: {}",
                hydration_failure_context,
                error.message
            );
            fallback_state
        }
    };
    save_license_state(app, &state, save_reason)?;

    if let Err(e) = crate::commands::config::sync_pipeline_config(app.clone()) {
        log::warn!(
            "Auth updated session but failed to sync pipeline config: {}",
            e
        );
    }

    Ok(state)
}

async fn persist_session_and_hydrate_license_state(
    app: &AppHandle,
    auth: SupabaseSessionAuthResponse,
    hydration_method: Method,
    hydration_path: &str,
    save_reason: &str,
    hydration_failure_context: &str,
) -> CommandResult<LicenseState> {
    persist_session_material_and_hydrate_license_state(
        app,
        SessionMaterial {
            access_token: auth.access_token,
            refresh_token: auth.refresh_token,
        },
        auth.user.id,
        auth.user.email,
        hydration_method,
        hydration_path,
        save_reason,
        hydration_failure_context,
    )
    .await
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

fn build_public_auth_page_url(
    auth_page_url: &str,
    callback_url: &str,
    state: &str,
) -> CommandResult<Url> {
    let mut url = Url::parse(auth_page_url).map_err(|e| {
        CommandError::new(format!("Public auth page URL is invalid: {e}"), "auth")
            .with_code("auth_not_configured")
    })?;

    if !matches!(url.scheme(), "http" | "https") {
        return Err(
            CommandError::new("Public auth page URL must use http or https.", "auth")
                .with_code("auth_not_configured"),
        );
    }

    url.query_pairs_mut()
        .append_pair("desktop_callback_url", callback_url)
        .append_pair("desktop_state", state)
        .append_pair("desktop_return_mode", "session_handoff");

    Ok(url)
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

fn parse_http_request(request: &str) -> Option<(&str, &str, &str)> {
    let (head, body) = request
        .split_once("\r\n\r\n")
        .or_else(|| request.split_once("\n\n"))?;
    let mut parts = head.lines().next()?.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    Some((method, target, body))
}

fn parse_loopback_callback_url(target: &str) -> Result<Url, String> {
    Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| "The browser callback URL was invalid.".to_string())
}

fn extract_auth_code_from_callback_target(
    target: &str,
    expected_state: &str,
) -> Result<String, String> {
    let callback_url = parse_loopback_callback_url(target)?;

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

fn extract_public_auth_session_from_callback_request(
    target: &str,
    body: &str,
    expected_state: &str,
) -> Result<PublicAuthSessionCallback, String> {
    let callback_url = parse_loopback_callback_url(target)?;
    if callback_url.path() != LOOPBACK_CALLBACK_PATH {
        return Err("The browser returned to an unexpected callback path.".to_string());
    }

    let trimmed_body = body.trim();
    let payload = if trimmed_body.starts_with('{') {
        serde_json::from_str::<PublicAuthSessionCallbackRequest>(trimmed_body)
            .map_err(|_| "The browser callback payload was malformed.".to_string())?
    } else {
        serde_urlencoded::from_str::<PublicAuthSessionCallbackRequest>(trimmed_body)
            .map_err(|_| "The browser callback payload was malformed.".to_string())?
    };
    let PublicAuthSessionCallbackRequest {
        state,
        access_token,
        refresh_token,
        user_id,
        email,
    } = payload;

    if state != expected_state {
        return Err("The browser sign-in state did not match the original request.".to_string());
    }

    let access_token = access_token.trim();
    if access_token.is_empty() {
        return Err(
            "The browser callback did not include an access token for desktop sign-in.".to_string(),
        );
    }
    let refresh_token = refresh_token.trim();
    if refresh_token.is_empty() {
        return Err(
            "The browser callback did not include a refresh token for desktop sign-in.".to_string(),
        );
    }
    let user_id = user_id.trim();
    if user_id.is_empty() {
        return Err(
            "The browser callback did not include a user id for desktop sign-in.".to_string(),
        );
    }

    Ok(PublicAuthSessionCallback {
        session: SessionMaterial {
            access_token: access_token.to_string(),
            refresh_token: refresh_token.to_string(),
        },
        user_id: user_id.to_string(),
        email: email.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }),
    })
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
        body.len()
    );

    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.flush().await;
}

async fn wait_for_browser_auth_callback(
    listener: TcpListener,
    expected_state: &str,
) -> CommandResult<BrowserAuthCallback> {
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
    let result = parse_http_request(&request)
        .ok_or_else(|| "The browser callback was malformed.".to_string())
        .and_then(|(method, target, body)| {
            if method.eq_ignore_ascii_case("GET") {
                return extract_auth_code_from_callback_target(target, expected_state)
                    .map(BrowserAuthCallback::AuthorizationCode);
            }

            if method.eq_ignore_ascii_case("POST") {
                return extract_public_auth_session_from_callback_request(
                    target,
                    body,
                    expected_state,
                )
                .map(BrowserAuthCallback::Session);
            }

            Err("The browser callback used an unsupported HTTP method.".to_string())
        });

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

fn resolve_signup_credentials(request: &SignupRequest) -> CommandResult<(String, String)> {
    let email = request.email.trim();
    let password = request.password.as_str();

    if email.is_empty() || password.is_empty() {
        return Err(CommandError::new(
            "Email and password are both required for account creation.",
            "auth",
        )
        .with_code("auth_credentials_missing"));
    }

    Ok((email.to_string(), password.to_string()))
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

fn refresh_error_requires_reauthentication(detail: &str) -> bool {
    let lower = detail.to_ascii_lowercase();

    lower.contains("refresh_token_not_found")
        || lower.contains("refresh token not found")
        || lower.contains("invalid refresh token")
}

fn build_license_refresh_failure_error(
    detail: impl Into<String>,
    code: Option<&str>,
) -> CommandError {
    let detail = detail.into();

    if refresh_error_requires_reauthentication(&detail) {
        return CommandError::new(
            "Your saved session is no longer valid. Please sign in again to refresh managed access.",
            "auth",
        )
        .with_code("reauth_required")
        .with_details(detail);
    }

    CommandError::new(
        "Latest entitlement and organization access data could not be loaded.",
        "auth",
    )
    .with_code(code.unwrap_or("auth_refresh_failed"))
    .with_details(detail)
    .with_retryable(true)
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

fn auth_entitlements_for_state(state: &LicenseState) -> Vec<String> {
    match state.tier {
        LicenseTier::Community => Vec::new(),
        LicenseTier::Personal => vec!["managed_inference".to_string()],
        LicenseTier::Enterprise => {
            let managed_inference_enabled = !matches!(
                state.org.as_ref().and_then(|org| org.inference_mode),
                Some(crate::licensing::OrgInferenceMode::OrgByok)
            );

            let mut entitlements = vec!["enterprise_policy".to_string()];
            if managed_inference_enabled {
                entitlements.insert(0, "managed_inference".to_string());
            }
            entitlements
        }
    }
}

fn auth_context_org_id(state: &LicenseState) -> Option<String> {
    if let Some(org) = &state.org {
        return Some(org.org_id.clone());
    }

    if matches!(state.tier, LicenseTier::Community | LicenseTier::Personal) {
        return state
            .user_id
            .as_ref()
            .map(|user_id| format!("personal:{user_id}"));
    }

    None
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
        org_id: auth_context_org_id(state),
        entitlements: if session_usable {
            auth_entitlements_for_state(state)
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
        return persist_session_and_hydrate_license_state(
            &app,
            auth,
            Method::GET,
            "/v1/license/state",
            "password_login_success",
            "Password login",
        )
        .await;
    }

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
    let browser_auth_url = if request.auth_provider.is_none() {
        if let Some(auth_page_url) = public_auth_page_url() {
            // Prefer the hosted Kolboo auth page when it is configured so browser
            // sign-in can support sign-up, magic-link, and post-confirmation return
            // to desktop. Explicit auth_provider requests still opt into direct
            // provider OAuth below.
            build_public_auth_page_url(&auth_page_url, &redirect_uri, &state_token)?
        } else {
            let (supabase_url, _) = supabase_auth_config()?;
            let auth_provider = resolve_supabase_auth_provider(None)?;
            build_supabase_authorize_url(
                &supabase_url,
                &auth_provider,
                &redirect_uri,
                &state_token,
                &code_challenge,
            )?
        }
    } else {
        let (supabase_url, _) = supabase_auth_config()?;
        let auth_provider = resolve_supabase_auth_provider(request.auth_provider.as_deref())?;
        build_supabase_authorize_url(
            &supabase_url,
            &auth_provider,
            &redirect_uri,
            &state_token,
            &code_challenge,
        )?
    };

    open::that(browser_auth_url.as_str()).map_err(|e| {
        CommandError::new(format!("Failed to open browser for sign in: {e}"), "auth")
            .with_code("auth_browser_open_failed")
    })?;

    match wait_for_browser_auth_callback(listener, &state_token).await? {
        BrowserAuthCallback::AuthorizationCode(auth_code) => {
            let auth =
                exchange_supabase_auth_code(auth_code.as_str(), code_verifier.as_str()).await?;
            persist_session_and_hydrate_license_state(
                &app,
                auth,
                Method::GET,
                "/v1/license/state",
                "login_success",
                "Browser login",
            )
            .await
        }
        BrowserAuthCallback::Session(callback) => {
            persist_session_material_and_hydrate_license_state(
                &app,
                callback.session,
                callback.user_id,
                callback.email,
                Method::GET,
                "/v1/license/state",
                "browser_handoff_login_success",
                "Browser auth handoff",
            )
            .await
        }
    }
}

#[tauri::command]
pub async fn license_sign_up(
    app: AppHandle,
    request: SignupRequest,
) -> CommandResult<SignupResponse> {
    let (email, password) = resolve_signup_credentials(&request)?;
    let signup = sign_up_supabase_with_password(email.as_str(), password.as_str()).await?;
    let response_email = signup.display_email().or_else(|| Some(email.clone()));

    if let Some(auth) = signup.to_session_auth_response() {
        let state = persist_session_and_hydrate_license_state(
            &app,
            auth,
            Method::GET,
            "/v1/license/state",
            "password_signup_success",
            "Password signup",
        )
        .await?;

        return Ok(SignupResponse {
            state,
            confirmation_required: false,
            email: response_email,
        });
    }

    // When Supabase email confirmation is enabled, /signup creates (or faux-creates
    // for duplicate protection) the user but does not return session tokens. Keep the
    // local license state unchanged until the user confirms and signs in.
    let state = load_license_state(&app)?;
    Ok(SignupResponse {
        state,
        confirmation_required: true,
        email: response_email,
    })
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

    let (next, save_reason, refresh_error) = if failed {
        (apply_refresh_failure(current, now), "refresh_failed", None)
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

                let refreshed_user_id = refresh.user.as_ref().map(|user| user.id.clone());
                let refreshed_email = refresh.user.as_ref().and_then(|user| user.email.clone());

                match fetch_license_state_from_api(
                    &refreshed_session.access_token,
                    Method::POST,
                    "/v1/license/refresh",
                )
                .await
                {
                    Ok(hydrated) => (
                        merge_hydrated_identity(
                            hydrated,
                            refreshed_user_id.or_else(|| current.user_id.clone()),
                            refreshed_email.or_else(|| current.email.clone()),
                        ),
                        "refresh_success",
                        None,
                    ),
                    Err(error) => {
                        log::warn!(
                            "License refresh updated the Supabase session, but api-edge hydration failed: {}",
                            error.message
                        );

                        let fallback = apply_refresh_failure(current, now);
                        let next =
                            merge_hydrated_identity(fallback, refreshed_user_id, refreshed_email);
                        let failure = build_license_refresh_failure_error(
                            error.message.to_string(),
                            error.code.as_deref(),
                        );

                        (next, "refresh_failed", Some(failure))
                    }
                }
            }
            Err(err) => {
                log::warn!("License refresh token exchange failed: {}", err.message);

                // If the refresh token is stale but the existing access token is still
                // usable, reuse it to hydrate the latest entitlement state so recent
                // upgrades/downgrades can still land locally without pretending the
                // session rotation succeeded.
                match fetch_license_state_from_api(
                    &session.access_token,
                    Method::POST,
                    "/v1/license/refresh",
                )
                .await
                {
                    Ok(hydrated) => {
                        log::warn!(
                            "License refresh reused the existing access token because session rotation failed"
                        );
                        (
                            merge_hydrated_identity(
                                hydrated,
                                current.user_id.clone(),
                                current.email.clone(),
                            ),
                            "refresh_success_existing_session",
                            None,
                        )
                    }
                    Err(hydration_error) => {
                        log::warn!(
                            "License refresh fallback hydration with existing access token failed: {}",
                            hydration_error.message
                        );

                        let next = apply_refresh_failure(current, now);
                        let detail = format!(
                            "{}; fallback hydration with existing access token also failed: {}",
                            err.message, hydration_error.message,
                        );
                        let failure = build_license_refresh_failure_error(
                            detail,
                            err.code.as_deref().or(hydration_error.code.as_deref()),
                        );

                        (next, "refresh_failed", Some(failure))
                    }
                }
            }
        }
    };

    save_license_state(&app, &next, save_reason)?;

    if let Err(e) = crate::commands::config::sync_pipeline_config(app.clone()) {
        log::warn!(
            "License refresh updated local state but failed to sync pipeline config: {}",
            e
        );
    }

    if let Some(error) = refresh_error {
        return Err(error);
    }

    Ok(next)
}

#[tauri::command]
pub async fn license_get_management_url(app: AppHandle) -> CommandResult<String> {
    let state = load_license_state(&app)?;
    if !state.portal_available {
        return Err(CommandError::new(
            "Billing portal is not available in this environment yet.",
            "auth",
        )
        .with_code("portal_unavailable"));
    }

    let session = load_session_material(&app).ok_or_else(|| {
        CommandError::new(
            "Sign in is required before opening account management.",
            "auth",
        )
        .with_code("reauth_required")
    })?;

    let response = fetch_license_portal_url_from_api(&session.access_token).await?;
    if !response.available {
        return Err(CommandError::new(
            "Billing portal is not available in this environment yet.",
            "auth",
        )
        .with_code("portal_unavailable"));
    }

    response.url.ok_or_else(|| {
        CommandError::new("Billing portal response did not include a URL.", "auth")
            .with_code("portal_unavailable")
    })
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};
    use reqwest::Url;

    use crate::licensing::{LicenseTier, OrgContext, OrgInferenceMode, PolicyStatus};

    use super::{
        build_auth_context, build_license_refresh_failure_error, build_pkce_code_challenge,
        build_public_auth_page_url, build_session_exchange_placeholder_response,
        build_supabase_authorize_url, extract_auth_code_from_callback_target,
        extract_public_auth_session_from_callback_request, refresh_error_requires_reauthentication,
        resolve_password_login_credentials, resolve_signup_credentials, SupabaseSignupAuthResponse,
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
    fn auth_context_omits_managed_inference_for_enterprise_byok() {
        let now = Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 0).unwrap();
        let mut state = crate::licensing::LicenseState::signed_out(now);
        state.status = crate::licensing::LicenseStatus::Active;
        state.user_id = Some("user-123".to_string());
        state.tier = LicenseTier::Enterprise;
        state.org = Some(OrgContext {
            org_id: "org-123".to_string(),
            org_name: "Kolboo Shared Dev Pilot Org".to_string(),
            inference_mode: Some(OrgInferenceMode::OrgByok),
        });

        let context = build_auth_context(&state, true, Some("https://issuer.test".to_string()));

        assert!(context.authenticated);
        assert_eq!(context.entitlements, vec!["enterprise_policy".to_string()]);
        assert_eq!(context.org_id.as_deref(), Some("org-123"));
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
    fn public_auth_page_url_includes_desktop_handoff_parameters() {
        let url = build_public_auth_page_url(
            "https://admin.kolboo.test/login?mode=sign-in",
            "http://127.0.0.1:43123/auth/callback",
            "state-123",
        )
        .expect("public auth page url");
        let parsed = Url::parse(url.as_ref()).expect("parse public auth url");
        let params = parsed.query_pairs().into_owned().collect::<Vec<_>>();

        assert!(params.contains(&("mode".to_string(), "sign-in".to_string())));
        assert!(params.contains(&(
            "desktop_callback_url".to_string(),
            "http://127.0.0.1:43123/auth/callback".to_string(),
        )));
        assert!(params.contains(&("desktop_state".to_string(), "state-123".to_string())));
        assert!(params.contains(&(
            "desktop_return_mode".to_string(),
            "session_handoff".to_string(),
        )));
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
    fn signup_credentials_trim_email_and_preserve_password() {
        let request = crate::licensing::SignupRequest {
            email: " new@example.com ".to_string(),
            password: " secret-with-space ".to_string(),
        };

        let credentials = resolve_signup_credentials(&request).expect("valid credentials");

        assert_eq!(credentials.0, "new@example.com");
        assert_eq!(credentials.1, " secret-with-space ");
    }

    #[test]
    fn signup_credentials_reject_empty_values() {
        let request = crate::licensing::SignupRequest {
            email: " ".to_string(),
            password: "secret".to_string(),
        };

        let error = resolve_signup_credentials(&request).expect_err("empty email should fail");

        assert_eq!(error.code.as_deref(), Some("auth_credentials_missing"));
    }

    #[test]
    fn signup_response_builds_session_when_tokens_are_returned() {
        let response: SupabaseSignupAuthResponse = serde_json::from_value(serde_json::json!({
            "access_token": "access-token",
            "refresh_token": "refresh-token",
            "user": {
                "id": "user-123",
                "email": "new@example.com"
            }
        }))
        .expect("signup response");

        let session = response
            .to_session_auth_response()
            .expect("auto-confirmed signup should include a session");

        assert_eq!(session.access_token, "access-token");
        assert_eq!(session.refresh_token, "refresh-token");
        assert_eq!(session.user.id, "user-123");
        assert_eq!(session.user.email.as_deref(), Some("new@example.com"));
    }

    #[test]
    fn signup_response_without_tokens_requires_confirmation() {
        let response: SupabaseSignupAuthResponse = serde_json::from_value(serde_json::json!({
            "id": "user-123",
            "email": "new@example.com"
        }))
        .expect("signup response");

        assert!(response.to_session_auth_response().is_none());
        assert_eq!(response.display_email().as_deref(), Some("new@example.com"));
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
    fn session_callback_parser_extracts_session_material_when_state_matches() {
        let callback = extract_public_auth_session_from_callback_request(
            "/auth/callback",
            "state=expected-state&access_token=access-123&refresh_token=refresh-456&user_id=user-789&email=user%40example.com",
            "expected-state",
        )
        .expect("session callback");

        assert_eq!(callback.session.access_token, "access-123");
        assert_eq!(callback.session.refresh_token, "refresh-456");
        assert_eq!(callback.user_id, "user-789");
        assert_eq!(callback.email.as_deref(), Some("user@example.com"));
    }

    #[test]
    fn session_callback_parser_rejects_state_mismatch() {
        let error = extract_public_auth_session_from_callback_request(
            "/auth/callback",
            "state=wrong-state&access_token=access-123&refresh_token=refresh-456&user_id=user-789",
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

    #[test]
    fn refresh_error_reauth_detection_matches_invalid_refresh_token_shapes() {
        assert!(refresh_error_requires_reauthentication(
            "Session refresh failed (400 Bad Request): {\"error_code\":\"refresh_token_not_found\"}",
        ));
        assert!(refresh_error_requires_reauthentication(
            "Session refresh failed (400 Bad Request): Invalid Refresh Token",
        ));
        assert!(!refresh_error_requires_reauthentication(
            "Session refresh request failed: connection reset by peer",
        ));
    }

    #[test]
    fn build_refresh_failure_error_surfaces_reauth_for_invalid_refresh_tokens() {
        let error = build_license_refresh_failure_error(
            "Session refresh failed (400 Bad Request): Invalid Refresh Token: Refresh Token Not Found",
            Some("auth_refresh_failed"),
        );

        assert_eq!(error.code.as_deref(), Some("reauth_required"));
        assert!(error.message.contains("Please sign in again"));
        assert!(error
            .details
            .as_deref()
            .unwrap_or_default()
            .contains("Invalid Refresh Token"));
        assert_eq!(error.retryable, None);
    }

    #[test]
    fn build_refresh_failure_error_marks_transient_failures_retryable() {
        let error = build_license_refresh_failure_error(
            "License hydration failed (503 Service Unavailable)",
            Some("license_state_unavailable"),
        );

        assert_eq!(error.code.as_deref(), Some("license_state_unavailable"));
        assert!(error
            .message
            .contains("Latest entitlement and organization access data could not be loaded"));
        assert_eq!(error.retryable, Some(true));
    }
}
