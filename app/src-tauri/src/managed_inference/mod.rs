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
    #[serde(default)]
    pub catalog_version: Option<String>,
}

#[cfg(desktop)]
pub(crate) async fn fetch_managed_model_catalog(
    client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
    cloudflare_access: Option<(&str, &str)>,
) -> crate::commands::CommandResult<ManagedModelCatalogResponse> {
    use crate::commands::CommandError;
    let url = format!(
        "{}/v1/managed/models",
        base_url.trim().trim_end_matches('/')
    );
    reqwest::Url::parse(&url)
        .map_err(|_| "Managed inference gateway URL is invalid".to_string())?;

    let mut request = client
        .get(url)
        .bearer_auth(access_token)
        .header("x-kolboo-managed-capabilities", "ivritai-v1");
    if let Some((client_id, client_secret)) = cloudflare_access {
        request = request
            .header("CF-Access-Client-Id", client_id)
            .header("CF-Access-Client-Secret", client_secret);
    }

    let response = request
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|error| {
            crate::sentry_init::report_account_failure(
                "model_catalog",
                if error.is_timeout() {
                    "timeout"
                } else {
                    "network"
                },
                None,
            );
            CommandError::new(
                "Managed models could not be reached. Please try again.",
                "network",
            )
            .with_code("catalog_unavailable")
            .with_retryable(true)
        })?;
    let status = response.status();
    if !status.is_success() {
        crate::sentry_init::report_account_failure(
            "model_catalog",
            "http_rejected",
            Some(status.as_u16()),
        );
        return Err(CommandError::new(
            format!("Managed model catalog request failed ({})", status.as_u16()),
            "auth",
        )
        .with_code(if status == reqwest::StatusCode::UNAUTHORIZED {
            "catalog_unauthorized"
        } else {
            "catalog_unavailable"
        }));
    }

    response
        .json::<ManagedModelCatalogResponse>()
        .await
        .map_err(|_| {
            crate::sentry_init::report_account_failure(
                "model_catalog",
                "invalid_response",
                Some(status.as_u16()),
            );
            CommandError::new("Managed model catalog response was invalid.", "network")
                .with_code("catalog_invalid_response")
        })
}

#[cfg(desktop)]
#[tauri::command]
pub async fn managed_inference_get_models(
    app: tauri::AppHandle,
) -> Result<ManagedModelCatalogResponse, String> {
    use std::time::Duration;
    use tauri_plugin_store::StoreExt;

    let base_url =
        crate::public_config::read(&["TAURI_MANAGED_INFERENCE_GATEWAY_URL", "TAURI_API_BASE_URL"])
            .ok_or_else(|| "Managed inference gateway URL is not configured".to_string())?;
    let access_token = crate::commands::licensing::managed_access_token(&app, None)
        .await
        .map_err(|error| error.to_string())?;
    let proxy_settings = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("proxy_settings"))
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let client =
        crate::network::build_http_client_with_timeout(&proxy_settings, Duration::from_secs(20))?;
    let access = crate::http::cloudflare_access_headers_for_url(&base_url);
    let cloudflare_access = access
        .as_ref()
        .map(|(id, secret)| (id.as_str(), secret.as_str()));

    let result =
        fetch_managed_model_catalog(&client, &base_url, &access_token, cloudflare_access).await;
    match result {
        Err(error) if error.code.as_deref() == Some("catalog_unauthorized") => {
            let token = crate::commands::licensing::managed_access_token(&app, Some(&access_token))
                .await
                .map_err(|error| error.to_string())?;
            fetch_managed_model_catalog(&client, &base_url, &token, cloudflare_access)
                .await
                .map_err(|error| error.to_string())
        }
        other => other.map_err(|error| error.to_string()),
    }
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
    async fn catalog_errors_are_classified_without_provider_content() {
        for (status, code) in [
            (401, "catalog_unauthorized"),
            (403, "catalog_unavailable"),
            (503, "catalog_unavailable"),
            (200, "catalog_invalid_response"),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(status).set_body_string("PRIVATE response"))
                .mount(&server)
                .await;
            let error = fetch_managed_model_catalog(
                &reqwest::Client::new(),
                &server.uri(),
                "PRIVATE token",
                None,
            )
            .await
            .unwrap_err();
            assert_eq!(error.code.as_deref(), Some(code));
            assert!(!error.to_string().contains("PRIVATE"));
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let error =
            fetch_managed_model_catalog(&reqwest::Client::new(), &url, "PRIVATE token", None)
                .await
                .unwrap_err();
        assert_eq!(error.code.as_deref(), Some("catalog_unavailable"));
        assert!(!error.to_string().contains(&url));
    }

    #[tokio::test]
    async fn fetches_catalog_with_session_and_cloudflare_access_headers() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/managed/models"))
            .and(header("authorization", "Bearer session-token"))
            .and(header("x-kolboo-managed-capabilities", "ivritai-v1"))
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
