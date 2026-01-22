//! Backup/export/import helpers.
//!
//! This module intentionally excludes secrets from exports.
//!
//! - Settings live in `settings.json` (tauri-plugin-store)
//! - API keys and other secrets live in OS secure storage (see `crate::secrets`)

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager};

#[cfg(desktop)]
use crate::settings::store::{get_settings_store_or_err, SettingsReadMode};

use crate::commands::{CommandError, CommandResult};
use crate::events;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SettingsBackupPayload {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub settings: serde_json::Value,
}

#[cfg(desktop)]
fn should_exclude_setting_key_from_backup(key: &str) -> bool {
    // Extra defense in depth: older installs might still have plaintext keys.
    if key.ends_with("_api_key") {
        return true;
    }

    // Avoid a circular/self-referential backup pointer.
    if key == "github_backup_gist_id" {
        return true;
    }

    // Never back up router embeddings cache (large, non-valuable, regeneratable).
    if key == crate::router_embeddings_cache::ROUTER_EMBEDDINGS_STORE_KEY {
        return true;
    }

    false
}

#[cfg(desktop)]
fn load_settings_json_from_disk(app: &AppHandle) -> CommandResult<serde_json::Value> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {e}"))?;
    let settings_path = app_data_dir.join("settings.json");

    if !settings_path.exists() {
        return Ok(serde_json::json!({}));
    }

    let bytes = std::fs::read(&settings_path).map_err(|e| {
        format!(
            "Failed to read settings file {}: {}",
            settings_path.display(),
            e
        )
    })?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
        format!(
            "Failed to parse settings file {}: {}",
            settings_path.display(),
            e
        )
    })?;
    Ok(v)
}

#[cfg(desktop)]
fn sanitize_settings_for_backup(raw: serde_json::Value) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    let obj = raw.as_object().cloned().unwrap_or_default();
    for (k, v) in obj {
        if should_exclude_setting_key_from_backup(k.as_str()) {
            continue;
        }
        out.insert(k, v);
    }
    serde_json::Value::Object(out)
}

#[cfg(desktop)]
fn build_backup_payload(app: &AppHandle) -> CommandResult<SettingsBackupPayload> {
    let raw = load_settings_json_from_disk(app)?;
    let settings = sanitize_settings_for_backup(raw);
    Ok(SettingsBackupPayload {
        format: "kolboo-settings-backup".to_string(),
        version: 1,
        exported_at: chrono::Utc::now().to_rfc3339(),
        settings,
    })
}

/// Export settings backup as a JSON string (pretty-printed).
#[cfg(desktop)]
#[tauri::command]
pub fn export_settings_backup_json(app: AppHandle) -> CommandResult<String> {
    let payload = build_backup_payload(&app)?;
    serde_json::to_string_pretty(&payload).map_err(|e| e.to_string().into())
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn export_settings_backup_json(_app: AppHandle) -> CommandResult<String> {
    Ok("{}".to_string())
}

/// Export settings backup to a file.
#[cfg(desktop)]
#[tauri::command]
pub fn export_settings_backup_to_file(app: AppHandle, path: String) -> CommandResult<()> {
    let payload = build_backup_payload(&app)?;
    let json = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;

    let path = PathBuf::from(path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, json)
        .map_err(|e| format!("Failed to write backup file {}: {}", path.display(), e))?;
    Ok(())
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn export_settings_backup_to_file(_app: AppHandle, _path: String) -> CommandResult<()> {
    Ok(())
}

#[cfg(desktop)]
fn import_settings_from_value(app: &AppHandle, value: serde_json::Value) -> CommandResult<()> {
    let settings_obj = match value {
        serde_json::Value::Object(o) => o,
        _ => return Err("Backup settings must be an object".to_string().into()),
    };

    let store = get_settings_store_or_err(app, SettingsReadMode::Cached)?;

    for (k, v) in settings_obj {
        if should_exclude_setting_key_from_backup(k.as_str()) {
            continue;
        }
        store.set(k, v);
    }

    store.save().map_err(|e| e.to_string())?;

    // Seed any newly-added defaults (and migrate legacy secrets best-effort).
    let _ = crate::settings::defaults::ensure_default_settings(app);

    // Best-effort: apply runtime config immediately.
    let _ = crate::commands::config::sync_pipeline_config(app.clone());

    // Notify other windows to refresh cached settings.
    let _ = app.emit(
        events::EVENT_SETTINGS_CHANGED,
        crate::SettingsChangedPayload::new(),
    );

    Ok(())
}

/// Import settings backup from a JSON string.
///
/// Supports:
/// - a `SettingsBackupPayload` wrapper
/// - or a raw settings object `{ ... }`
#[cfg(desktop)]
#[tauri::command]
pub fn import_settings_backup_json(app: AppHandle, json: String) -> CommandResult<()> {
    let v: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;

    // Wrapper format
    if let Some(settings) = v.get("settings").cloned().filter(|s| s.is_object()) {
        return import_settings_from_value(&app, settings);
    }

    // Raw object
    import_settings_from_value(&app, v)
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn import_settings_backup_json(_app: AppHandle, _json: String) -> CommandResult<()> {
    Ok(())
}

/// Import settings backup from a file.
#[cfg(desktop)]
#[tauri::command]
pub fn import_settings_backup_from_file(app: AppHandle, path: String) -> CommandResult<()> {
    let path = PathBuf::from(path);
    let bytes = std::fs::read(&path)
        .map_err(|e| format!("Failed to read backup file {}: {}", path.display(), e))?;
    let json = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    import_settings_backup_json(app, json)
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn import_settings_backup_from_file(_app: AppHandle, _path: String) -> CommandResult<()> {
    Ok(())
}

// ---------------------------------------------------------------------------
// GitHub Gist backup (token stored in secure storage)
// ---------------------------------------------------------------------------

#[cfg(desktop)]
const GITHUB_GIST_TOKEN_KEY: &str = "github_gist_token";

#[cfg(desktop)]
const GITHUB_GIST_BACKUP_FILE_NAME: &str = "kolboo-settings-backup.json";

#[cfg(desktop)]
fn github_client() -> reqwest::Client {
    crate::network::build_plain_http_client_with_user_agent("kolboo")
}

#[cfg(desktop)]
fn github_token(app: &AppHandle) -> CommandResult<String> {
    let token = crate::secrets::get_secret(app, GITHUB_GIST_TOKEN_KEY)
        .ok_or_else(|| CommandError::new("GitHub token not configured", "secrets"))?;
    Ok(token)
}

#[cfg(desktop)]
#[tauri::command]
pub fn github_backup_has_token(app: AppHandle) -> bool {
    crate::secrets::has_secret(&app, GITHUB_GIST_TOKEN_KEY)
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn github_backup_has_token(_app: AppHandle) -> bool {
    false
}

#[cfg(desktop)]
#[tauri::command]
pub fn github_backup_set_token(app: AppHandle, token: String) -> CommandResult<()> {
    crate::secrets::set_secret(&app, GITHUB_GIST_TOKEN_KEY, token.as_str()).map_err(Into::into)
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn github_backup_set_token(_app: AppHandle, _token: String) -> CommandResult<()> {
    Ok(())
}

#[cfg(desktop)]
#[tauri::command]
pub fn github_backup_clear_token(app: AppHandle) -> CommandResult<()> {
    crate::secrets::clear_secret(&app, GITHUB_GIST_TOKEN_KEY).map_err(Into::into)
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn github_backup_clear_token(_app: AppHandle) -> CommandResult<()> {
    Ok(())
}

#[cfg(desktop)]
#[tauri::command]
pub async fn github_backup_push_to_gist(
    app: AppHandle,
    gist_id: Option<String>,
) -> CommandResult<String> {
    let token = github_token(&app)?;
    let payload = build_backup_payload(&app)?;
    let content = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;

    let client = github_client();
    let url = if let Some(id) = gist_id.as_deref().filter(|s| !s.trim().is_empty()) {
        format!("https://api.github.com/gists/{}", id.trim())
    } else {
        "https://api.github.com/gists".to_string()
    };

    let body = if gist_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .is_some()
    {
        serde_json::json!({
            "files": {
                GITHUB_GIST_BACKUP_FILE_NAME: {
                    "content": content
                }
            }
        })
    } else {
        serde_json::json!({
            "description": "Kolboo settings backup",
            "public": false,
            "files": {
                GITHUB_GIST_BACKUP_FILE_NAME: {
                    "content": content
                }
            }
        })
    };

    let req = if gist_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .is_some()
    {
        client.patch(url)
    } else {
        client.post(url)
    };

    let resp = req
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error ({}): {}", status, text).into());
    }

    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let id = v
        .get("id")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if id.trim().is_empty() {
        return Err("GitHub response missing gist id".to_string().into());
    }

    Ok(id)
}

#[cfg(not(desktop))]
#[tauri::command]
pub async fn github_backup_push_to_gist(
    _app: AppHandle,
    _gist_id: Option<String>,
) -> CommandResult<String> {
    Ok("".to_string())
}

#[cfg(desktop)]
#[tauri::command]
pub async fn github_backup_pull_from_gist(
    app: AppHandle,
    gist_id: String,
) -> CommandResult<String> {
    let token = github_token(&app)?;
    let id = gist_id.trim();
    if id.is_empty() {
        return Err("Missing gist id".to_string().into());
    }

    let client = github_client();
    let resp = client
        .get(format!("https://api.github.com/gists/{}", id))
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error ({}): {}", status, text).into());
    }

    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let files = v
        .get("files")
        .and_then(|f| f.as_object())
        .cloned()
        .unwrap_or_default();

    let file = files
        .get(GITHUB_GIST_BACKUP_FILE_NAME)
        .or_else(|| files.values().find(|vv| vv.get("content").is_some()))
        .ok_or_else(|| CommandError::new("Gist has no files", "backup"))?;

    let content = file
        .get("content")
        .and_then(|c| c.as_str())
        .ok_or_else(|| CommandError::new("Gist file content missing", "backup"))?;

    Ok(content.to_string())
}

#[cfg(not(desktop))]
#[tauri::command]
pub async fn github_backup_pull_from_gist(
    _app: AppHandle,
    _gist_id: String,
) -> CommandResult<String> {
    Ok("".to_string())
}
