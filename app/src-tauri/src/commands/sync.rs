use chrono::Utc;
use tauri::AppHandle;

use crate::commands::{CommandError, CommandResult};

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

#[cfg(desktop)]
const SYNC_ENABLED_KEY: &str = "cloud_sync_enabled";
#[cfg(desktop)]
const SYNC_AUTO_PUSH_KEY: &str = "cloud_sync_auto_push";
#[cfg(desktop)]
const SYNC_LAST_PUSHED_AT_KEY: &str = "cloud_sync_last_pushed_at";
#[cfg(desktop)]
const SYNC_LAST_PULLED_AT_KEY: &str = "cloud_sync_last_pulled_at";
#[cfg(desktop)]
const SYNC_LAST_ERROR_KEY: &str = "cloud_sync_last_error";
#[cfg(desktop)]
const SYNC_REMOTE_REVISION_KEY: &str = "cloud_sync_remote_revision";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncStatus {
    pub enabled: bool,
    pub auto_push_enabled: bool,
    pub endpoint_configured: bool,
    pub signed_in: bool,
    pub remote_revision: Option<String>,
    pub last_pushed_at: Option<String>,
    pub last_pulled_at: Option<String>,
    pub last_error: Option<String>,
}

#[cfg(desktop)]
fn api_base_url() -> Option<String> {
    let env_value = crate::commands::config::read_first_non_empty_env(&["TAURI_API_BASE_URL"])?;

    Some(env_value.trim_end_matches('/').to_string())
}

#[cfg(desktop)]
fn read_sync_status(app: &AppHandle) -> CommandResult<SyncStatus> {
    let store = app
        .store("settings.json")
        .map_err(|e| CommandError::unknown(format!("Failed to open settings store: {e}")))?;

    let enabled = store
        .get(SYNC_ENABLED_KEY)
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let auto_push_enabled = store
        .get(SYNC_AUTO_PUSH_KEY)
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let last_pushed_at = store
        .get(SYNC_LAST_PUSHED_AT_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_string()));
    let last_pulled_at = store
        .get(SYNC_LAST_PULLED_AT_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_string()));
    let last_error = store
        .get(SYNC_LAST_ERROR_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_string()));
    let remote_revision = store
        .get(SYNC_REMOTE_REVISION_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    Ok(SyncStatus {
        enabled,
        auto_push_enabled,
        endpoint_configured: api_base_url().is_some(),
        signed_in: crate::licensing::load_session_material(app).is_some(),
        remote_revision,
        last_pushed_at,
        last_pulled_at,
        last_error,
    })
}

#[cfg(desktop)]
fn store_sync_status(
    app: &AppHandle,
    pushed_at: Option<Option<String>>,
    pulled_at: Option<Option<String>>,
    last_error: Option<Option<String>>,
    remote_revision: Option<Option<String>>,
) -> CommandResult<()> {
    let store = app
        .store("settings.json")
        .map_err(|e| CommandError::unknown(format!("Failed to open settings store: {e}")))?;

    if let Some(value) = pushed_at {
        match value {
            Some(v) => store.set(SYNC_LAST_PUSHED_AT_KEY.to_string(), serde_json::json!(v)),
            None => {
                let _ = store.delete(SYNC_LAST_PUSHED_AT_KEY);
            }
        }
    }

    if let Some(value) = pulled_at {
        match value {
            Some(v) => store.set(SYNC_LAST_PULLED_AT_KEY.to_string(), serde_json::json!(v)),
            None => {
                let _ = store.delete(SYNC_LAST_PULLED_AT_KEY);
            }
        }
    }

    if let Some(value) = last_error {
        match value {
            Some(v) => store.set(SYNC_LAST_ERROR_KEY.to_string(), serde_json::json!(v)),
            None => {
                let _ = store.delete(SYNC_LAST_ERROR_KEY);
            }
        }
    }

    if let Some(value) = remote_revision {
        match value {
            Some(v) => store.set(SYNC_REMOTE_REVISION_KEY.to_string(), serde_json::json!(v)),
            None => {
                let _ = store.delete(SYNC_REMOTE_REVISION_KEY);
            }
        }
    }

    store
        .save()
        .map_err(|e| CommandError::unknown(format!("Failed to save settings store: {e}")))?;

    Ok(())
}

#[cfg(desktop)]
fn sync_client() -> reqwest::Client {
    crate::network::build_plain_http_client_with_user_agent("kolboo-sync")
}

#[cfg(desktop)]
fn sync_endpoint_url(base: &str) -> String {
    format!("{base}/v1/sync/settings")
}

#[cfg(desktop)]
pub(crate) async fn sync_push_settings_inner(app: &AppHandle) -> CommandResult<SyncStatus> {
    let status = read_sync_status(app)?;

    if !status.enabled {
        return Ok(status);
    }

    let base_url = api_base_url().ok_or_else(|| {
        CommandError::new("Cloud sync endpoint is not configured", "sync")
            .with_code("sync_not_configured")
    })?;

    let session = crate::licensing::load_session_material(app).ok_or_else(|| {
        CommandError::new("Sign in is required for cloud sync", "auth")
            .with_code("sync_requires_sign_in")
    })?;

    let payload = crate::commands::backup::build_backup_payload(app)?;
    let url = sync_endpoint_url(base_url.as_str());

    let resp = crate::http::with_cloudflare_access_headers_if_target(
        sync_client()
            .put(&url)
            .bearer_auth(session.access_token)
            .json(&payload),
        &url,
    )
    .send()
    .await
    .map_err(|e| {
        CommandError::new(format!("Cloud sync push failed: {e}"), "sync")
            .with_code("sync_push_failed")
    })?;

    if !resp.status().is_success() {
        let (status_code, text) = crate::http::status_and_text(resp).await;
        let msg = format!("Cloud sync push failed ({status_code}): {text}");
        let _ = store_sync_status(app, None, None, Some(Some(msg.clone())), None);
        return Err(CommandError::new(msg, "sync").with_code("sync_push_http_error"));
    }

    let now = Utc::now().to_rfc3339();
    let body: serde_json::Value = resp.json().await.unwrap_or_else(|_| serde_json::json!({}));
    let revision = body
        .get("revision")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    store_sync_status(app, Some(Some(now)), None, Some(None), Some(revision))?;

    read_sync_status(app)
}

#[cfg(desktop)]
pub(crate) async fn sync_pull_settings_inner(app: &AppHandle) -> CommandResult<SyncStatus> {
    let status = read_sync_status(app)?;

    if !status.enabled {
        return Ok(status);
    }

    let base_url = api_base_url().ok_or_else(|| {
        CommandError::new("Cloud sync endpoint is not configured", "sync")
            .with_code("sync_not_configured")
    })?;

    let session = crate::licensing::load_session_material(app).ok_or_else(|| {
        CommandError::new("Sign in is required for cloud sync", "auth")
            .with_code("sync_requires_sign_in")
    })?;

    let url = sync_endpoint_url(base_url.as_str());
    let resp = crate::http::with_cloudflare_access_headers_if_target(
        sync_client().get(&url).bearer_auth(session.access_token),
        &url,
    )
    .send()
    .await
    .map_err(|e| {
        CommandError::new(format!("Cloud sync pull failed: {e}"), "sync")
            .with_code("sync_pull_failed")
    })?;

    if !resp.status().is_success() {
        let (status_code, text) = crate::http::status_and_text(resp).await;
        let msg = format!("Cloud sync pull failed ({status_code}): {text}");
        let _ = store_sync_status(app, None, None, Some(Some(msg.clone())), None);
        return Err(CommandError::new(msg, "sync").with_code("sync_pull_http_error"));
    }

    let body: serde_json::Value = resp.json().await.map_err(|e| {
        CommandError::new(format!("Cloud sync payload parse failed: {e}"), "sync")
            .with_code("sync_pull_parse_error")
    })?;

    let settings_value = body
        .get("settings")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    crate::commands::backup::import_settings_from_value(app, settings_value)?;

    let now = Utc::now().to_rfc3339();
    let revision = body
        .get("revision")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    store_sync_status(app, None, Some(Some(now)), Some(None), Some(revision))?;

    read_sync_status(app)
}

#[cfg(desktop)]
#[tauri::command]
pub async fn sync_get_status(app: AppHandle) -> CommandResult<SyncStatus> {
    read_sync_status(&app)
}

#[cfg(not(desktop))]
#[tauri::command]
pub async fn sync_get_status(_app: AppHandle) -> CommandResult<SyncStatus> {
    Ok(SyncStatus {
        enabled: false,
        auto_push_enabled: true,
        endpoint_configured: false,
        signed_in: false,
        remote_revision: None,
        last_pushed_at: None,
        last_pulled_at: None,
        last_error: None,
    })
}

#[cfg(desktop)]
#[tauri::command]
pub async fn sync_push_settings(app: AppHandle) -> CommandResult<SyncStatus> {
    sync_push_settings_inner(&app).await
}

#[cfg(not(desktop))]
#[tauri::command]
pub async fn sync_push_settings(_app: AppHandle) -> CommandResult<SyncStatus> {
    Ok(SyncStatus {
        enabled: false,
        auto_push_enabled: true,
        endpoint_configured: false,
        signed_in: false,
        remote_revision: None,
        last_pushed_at: None,
        last_pulled_at: None,
        last_error: None,
    })
}

#[cfg(desktop)]
#[tauri::command]
pub async fn sync_pull_settings(app: AppHandle) -> CommandResult<SyncStatus> {
    sync_pull_settings_inner(&app).await
}

#[cfg(not(desktop))]
#[tauri::command]
pub async fn sync_pull_settings(_app: AppHandle) -> CommandResult<SyncStatus> {
    Ok(SyncStatus {
        enabled: false,
        auto_push_enabled: true,
        endpoint_configured: false,
        signed_in: false,
        remote_revision: None,
        last_pushed_at: None,
        last_pulled_at: None,
        last_error: None,
    })
}
