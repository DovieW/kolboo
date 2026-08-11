//! Desktop-side managed inference boundary helpers.

pub mod errors;

#[allow(dead_code)]
pub const CMD_STT_TRANSCRIBE: &str = "managed_inference_stt_transcribe";
#[allow(dead_code)]
pub const CMD_LLM_COMPLETE: &str = "managed_inference_llm_complete";
#[allow(dead_code)]
pub const CMD_USAGE_STATE: &str = "managed_inference_get_usage_state";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ManagedModel {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub capabilities: Vec<String>,
    pub default_for_provider: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ManagedModelCatalogResponse {
    pub models: Vec<ManagedModel>,
    pub request_id: String,
}

#[cfg(desktop)]
async fn fetch_managed_model_catalog(
    client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
    cloudflare_access: Option<(&str, &str)>,
) -> Result<ManagedModelCatalogResponse, String> {
    let url = format!(
        "{}/v1/managed/models",
        base_url.trim().trim_end_matches('/')
    );
    reqwest::Url::parse(&url)
        .map_err(|_| "Managed inference gateway URL is invalid".to_string())?;

    let mut request = client.get(url).bearer_auth(access_token);
    if let Some((client_id, client_secret)) = cloudflare_access {
        request = request
            .header("CF-Access-Client-Id", client_id)
            .header("CF-Access-Client-Secret", client_secret);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("Managed model catalog request failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "Managed model catalog request failed ({})",
            status.as_u16()
        ));
    }

    response
        .json::<ManagedModelCatalogResponse>()
        .await
        .map_err(|error| format!("Managed model catalog response was invalid: {error}"))
}

#[cfg(desktop)]
#[tauri::command]
pub async fn managed_inference_get_models(
    app: tauri::AppHandle,
) -> Result<ManagedModelCatalogResponse, String> {
    use std::time::Duration;
    use tauri_plugin_store::StoreExt;

    let base_url = crate::commands::config::read_first_non_empty_env(&[
        "TAURI_MANAGED_INFERENCE_GATEWAY_URL",
        "TAURI_API_BASE_URL",
    ])
    .ok_or_else(|| "Managed inference gateway URL is not configured".to_string())?;
    let access_token =
        crate::secrets::get_secret(&app, crate::secrets::AUTH_SESSION_ACCESS_TOKEN_KEY)
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| "Managed authentication is unavailable right now".to_string())?;
    let proxy_settings = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("proxy_settings"))
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let client =
        crate::network::build_http_client_with_timeout(&proxy_settings, Duration::from_secs(20))?;
    let cloudflare_client_id =
        crate::commands::config::read_first_non_empty_env(&["TAURI_CLOUDFLARE_ACCESS_CLIENT_ID"]);
    let cloudflare_client_secret = crate::commands::config::read_first_non_empty_env(&[
        "TAURI_CLOUDFLARE_ACCESS_CLIENT_SECRET",
    ]);
    let cloudflare_access = cloudflare_client_id
        .as_deref()
        .zip(cloudflare_client_secret.as_deref());

    fetch_managed_model_catalog(&client, &base_url, &access_token, cloudflare_access).await
}

#[cfg(not(desktop))]
#[tauri::command]
pub async fn managed_inference_get_models(
    _app: tauri::AppHandle,
) -> Result<ManagedModelCatalogResponse, String> {
    Err("Managed model discovery requires the desktop runtime".to_string())
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManagedInferenceMode {
    Managed,
    Byok,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagedErrorCategory {
    Unauthorized,
    Ineligible,
    OverQuota,
    TemporarilyUnavailable,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ManagedError {
    pub category: ManagedErrorCategory,
    pub code: String,
    pub message: String,
    pub request_id: Option<String>,
    pub retry_after_seconds: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct ManagedUsageCounter {
    pub metric: String,
    pub used: u64,
    pub limit: u64,
    pub warning_thresholds: Vec<u64>,
    pub window: String,
}

#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct ManagedUsageState {
    pub tier: crate::licensing::LicenseTier,
    pub mode: ManagedInferenceMode,
    pub counters: Vec<ManagedUsageCounter>,
}

fn mode_for_license(state: &crate::licensing::LicenseState) -> ManagedInferenceMode {
    match (state.tier, state.status) {
        (
            crate::licensing::LicenseTier::Personal,
            crate::licensing::LicenseStatus::Active | crate::licensing::LicenseStatus::Grace,
        ) => ManagedInferenceMode::Managed,
        _ => ManagedInferenceMode::Byok,
    }
}

#[cfg(desktop)]
#[tauri::command]
pub fn managed_inference_get_usage_state(
    app: tauri::AppHandle,
) -> Result<ManagedUsageState, String> {
    use chrono::Utc;
    use tauri_plugin_store::StoreExt;

    let store = app
        .store("settings.json")
        .map_err(|e| format!("Failed to open settings store: {e}"))?;
    let license_state =
        crate::licensing::normalize_license_state(store.get("license_state"), Utc::now());
    let mode = mode_for_license(&license_state);

    Ok(ManagedUsageState {
        tier: license_state.tier,
        mode,
        counters: vec![
            ManagedUsageCounter {
                metric: "stt_seconds".to_string(),
                used: license_state.usage.stt_seconds_used,
                limit: license_state.limits.stt_seconds_monthly,
                warning_thresholds: vec![50, 80, 95],
                window: "monthly".to_string(),
            },
            ManagedUsageCounter {
                metric: "llm_tokens".to_string(),
                used: license_state.usage.llm_tokens_used,
                limit: license_state.limits.llm_tokens_monthly,
                warning_thresholds: vec![50, 80, 95],
                window: "monthly".to_string(),
            },
            ManagedUsageCounter {
                metric: "managed_requests".to_string(),
                used: license_state.usage.requests_today,
                limit: license_state.limits.requests_per_day,
                warning_thresholds: vec![50, 80, 95],
                window: "daily".to_string(),
            },
        ],
    })
}

#[cfg(all(test, desktop))]
mod tests {
    use super::fetch_managed_model_catalog;
    use wiremock::{
        matchers::{header, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[tokio::test]
    async fn fetches_catalog_with_session_and_cloudflare_access_headers() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/managed/models"))
            .and(header("authorization", "Bearer session-token"))
            .and(header("CF-Access-Client-Id", "cf-client-id"))
            .and(header("CF-Access-Client-Secret", "cf-client-secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "models": [{
                    "id": "gpt-5-mini",
                    "display_name": "GPT-5 mini",
                    "provider": "openai",
                    "capabilities": ["chat_completions", "responses"],
                    "default_for_provider": false
                }],
                "request_id": "request-1"
            })))
            .mount(&server)
            .await;

        let catalog = fetch_managed_model_catalog(
            &reqwest::Client::new(),
            &server.uri(),
            "session-token",
            Some(("cf-client-id", "cf-client-secret")),
        )
        .await
        .expect("catalog request should succeed");

        assert_eq!(catalog.request_id, "request-1");
        assert_eq!(catalog.models.len(), 1);
        assert_eq!(catalog.models[0].id, "gpt-5-mini");
    }
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn managed_inference_get_usage_state(
    _app: tauri::AppHandle,
) -> Result<ManagedUsageState, String> {
    Ok(ManagedUsageState {
        tier: crate::licensing::LicenseTier::Community,
        mode: ManagedInferenceMode::Byok,
        counters: vec![],
    })
}
