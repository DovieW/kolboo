//! Tauri commands for secure secret storage.

use tauri::AppHandle;

use crate::commands::CommandResult;

/// Check whether an API key exists.
///
/// This reads from OS secure storage when available, with a legacy fallback to
/// `settings.json` during migration.
#[cfg(desktop)]
#[tauri::command]
pub async fn secrets_has_api_key(app: AppHandle, store_key: String) -> bool {
    match tauri::async_runtime::spawn_blocking(move || {
        crate::secrets::has_api_key(&app, store_key.as_str())
    })
    .await
    {
        Ok(value) => value,
        Err(error) => {
            log::error!("API-key availability check task failed: {error}");
            false
        }
    }
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn secrets_has_api_key(_app: AppHandle, _store_key: String) -> bool {
    false
}

/// Get an API key (if present).
#[cfg(desktop)]
#[tauri::command]
pub async fn secrets_get_api_key(app: AppHandle, store_key: String) -> Option<String> {
    match tauri::async_runtime::spawn_blocking(move || {
        crate::secrets::get_api_key(&app, store_key.as_str())
    })
    .await
    {
        Ok(value) => value,
        Err(error) => {
            log::error!("API-key retrieval task failed: {error}");
            None
        }
    }
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn secrets_get_api_key(_app: AppHandle, _store_key: String) -> Option<String> {
    None
}

/// Set an API key.
///
/// This writes to OS secure storage, and removes any legacy plaintext copy from
/// `settings.json`.
#[cfg(desktop)]
#[tauri::command]
pub async fn secrets_set_api_key(
    app: AppHandle,
    store_key: String,
    api_key: String,
) -> CommandResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::secrets::set_api_key(&app, store_key.as_str(), api_key.as_str())
    })
    .await
    .map_err(|error| format!("Secret-storage task failed: {error}"))?
    .map_err(Into::into)
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn secrets_set_api_key(
    _app: AppHandle,
    _store_key: String,
    _api_key: String,
) -> CommandResult<()> {
    Ok(())
}

/// Clear an API key.
#[cfg(desktop)]
#[tauri::command]
pub async fn secrets_clear_api_key(app: AppHandle, store_key: String) -> CommandResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::secrets::clear_api_key(&app, store_key.as_str())
    })
    .await
    .map_err(|error| format!("Secret-storage task failed: {error}"))?
    .map_err(Into::into)
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn secrets_clear_api_key(_app: AppHandle, _store_key: String) -> CommandResult<()> {
    Ok(())
}
