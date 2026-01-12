//! Tauri commands for secure secret storage.

use tauri::AppHandle;

/// Check whether an API key exists.
///
/// This reads from OS secure storage when available, with a legacy fallback to
/// `settings.json` during migration.
#[cfg(desktop)]
#[tauri::command]
pub fn secrets_has_api_key(app: AppHandle, store_key: String) -> bool {
	crate::secrets::has_api_key(&app, store_key.as_str())
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn secrets_has_api_key(_app: AppHandle, _store_key: String) -> bool {
	false
}

/// Get an API key (if present).
#[cfg(desktop)]
#[tauri::command]
pub fn secrets_get_api_key(app: AppHandle, store_key: String) -> Option<String> {
	crate::secrets::get_api_key(&app, store_key.as_str())
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
pub fn secrets_set_api_key(app: AppHandle, store_key: String, api_key: String) -> Result<(), String> {
	crate::secrets::set_api_key(&app, store_key.as_str(), api_key.as_str())
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn secrets_set_api_key(_app: AppHandle, _store_key: String, _api_key: String) -> Result<(), String> {
	Ok(())
}

/// Clear an API key.
#[cfg(desktop)]
#[tauri::command]
pub fn secrets_clear_api_key(app: AppHandle, store_key: String) -> Result<(), String> {
	crate::secrets::clear_api_key(&app, store_key.as_str())
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn secrets_clear_api_key(_app: AppHandle, _store_key: String) -> Result<(), String> {
	Ok(())
}
