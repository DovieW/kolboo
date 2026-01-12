//! Secure storage for secrets (API keys).
//!
//! Goal: keep API keys out of `settings.json` (plaintext at rest).
//! We store secrets in the OS keychain/credential manager when available,
//! and fall back to the store for legacy installs during migration.

#[cfg(desktop)]
use std::error::Error;

#[cfg(desktop)]
use tauri::AppHandle;

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

#[cfg(desktop)]
use keyring::Entry;

/// Known API key setting keys that historically lived in `settings.json`.
///
/// Keep this in sync with provider lists in:
/// - `app/src-tauri/src/commands/config.rs`
/// - `app/src/lib/apiKeys.ts`
#[cfg(desktop)]
pub const API_KEY_SETTING_KEYS: &[&str] = &[
	// STT providers
	"groq_api_key",
	"elevenlabs_api_key",
	"openai_api_key",
	"fireworks_api_key",
	"aquavoice_api_key",
	"assemblyai_api_key",
	"speechmatics_api_key",
	"deepgram_api_key",

	// LLM providers
	"cerebras_api_key",
	"openai_api_key",
	"fireworks_api_key",
	"gemini_api_key",
	"anthropic_api_key",
	"cohere_api_key",
	"groq_api_key",
];

#[cfg(desktop)]
const SERVICE_NAME: &str = "kolboo";

#[cfg(desktop)]
fn validate_api_key_store_key(store_key: &str) -> Result<(), String> {
	if !store_key.ends_with("_api_key") {
		return Err("Invalid key name".to_string());
	}

	// Keep the surface area tight: only allow lowercase letters, digits, and underscores.
	// (Keys are expected to look like `${provider}_api_key`.)
	for ch in store_key.chars() {
		let ok = matches!(ch, 'a'..='z' | '0'..='9' | '_');
		if !ok {
			return Err("Invalid key name".to_string());
		}
	}

	Ok(())
}

#[cfg(desktop)]
fn entry_for_key(store_key: &str) -> Result<Entry, String> {
	validate_api_key_store_key(store_key)?;
	Entry::new(SERVICE_NAME, store_key).map_err(|e| e.to_string())
}

/// Best-effort read of a legacy API key from `settings.json`.
///
/// This is only for backward compatibility during migration.
#[cfg(desktop)]
fn get_legacy_api_key_from_store(app: &AppHandle, store_key: &str) -> Option<String> {
	let raw = app.store("settings.json").ok()?.get(store_key)?;

	// Store values are JSON; accept either string values or stringified JSON.
	if let Some(s) = raw.as_str() {
		let trimmed = s.trim();
		if trimmed.is_empty() {
			None
		} else {
			Some(trimmed.to_string())
		}
	} else {
		serde_json::from_value::<String>(raw)
			.ok()
			.map(|s| s.trim().to_string())
			.filter(|s| !s.is_empty())
	}
}

#[cfg(desktop)]
pub fn has_api_key(app: &AppHandle, store_key: &str) -> bool {
	get_api_key(app, store_key).is_some()
}

/// Get an API key.
///
/// Order:
/// 1) OS keyring
/// 2) Legacy `settings.json` (during migration)
#[cfg(desktop)]
pub fn get_api_key(app: &AppHandle, store_key: &str) -> Option<String> {
	let entry = entry_for_key(store_key).ok()?;
	match entry.get_password() {
		Ok(s) => {
			let trimmed = s.trim();
			if trimmed.is_empty() {
				None
			} else {
				Some(trimmed.to_string())
			}
		}
		Err(keyring::Error::NoEntry) => get_legacy_api_key_from_store(app, store_key),
		Err(e) => {
			log::warn!("Failed to read API key from secure storage ({}): {}", store_key, e);
			get_legacy_api_key_from_store(app, store_key)
		}
	}
}

#[cfg(desktop)]
pub fn set_api_key(app: &AppHandle, store_key: &str, api_key: &str) -> Result<(), String> {
	let trimmed = api_key.trim();
	if trimmed.is_empty() {
		return Err("API key cannot be empty".to_string());
	}

	let entry = entry_for_key(store_key)?;
	entry.set_password(trimmed).map_err(|e| e.to_string())?;

	// Ensure plaintext keys are removed from the settings store.
	if let Ok(store) = app.store("settings.json") {
		store.delete(store_key);
		let _ = store.save();
	}

	Ok(())
}

#[cfg(desktop)]
pub fn clear_api_key(app: &AppHandle, store_key: &str) -> Result<(), String> {
	let entry = entry_for_key(store_key)?;
	match entry.delete_credential() {
		Ok(()) => {}
		Err(keyring::Error::NoEntry) => {}
		Err(e) => {
			return Err(e.to_string());
		}
	}

	// Also clear any legacy value that may remain.
	if let Ok(store) = app.store("settings.json") {
		store.delete(store_key);
		let _ = store.save();
	}

	Ok(())
}

/// One-time (best-effort) migration from `settings.json` to OS keyring.
///
/// Behavior:
/// - If a key exists in the store and not in secure storage, write it to secure storage.
/// - If secure storage already has a key, delete the store copy.
/// - If secure storage write fails, keep the store key (to avoid breaking existing users).
#[cfg(desktop)]
pub fn migrate_api_keys_from_store(app: &AppHandle) -> Result<(), Box<dyn Error>> {
	let store = app.store("settings.json")?;
	let mut dirty = false;

	for key in API_KEY_SETTING_KEYS {
		let Some(value) = store.get(key) else { continue };
		let Ok(s) = serde_json::from_value::<String>(value) else { continue };
		let trimmed = s.trim();
		if trimmed.is_empty() {
			continue;
		}

		let entry = match entry_for_key(key) {
			Ok(e) => e,
			Err(_) => continue,
		};

		match entry.get_password() {
			Ok(existing) => {
				if !existing.trim().is_empty() {
					// Secure storage already has it; remove the legacy plaintext copy.
					store.delete(key);
					dirty = true;
					continue;
				}
			}
			Err(keyring::Error::NoEntry) => {}
			Err(e) => {
				log::warn!("Failed checking secure storage for {}: {}", key, e);
				// Don't delete the store key if we can't safely confirm.
				continue;
			}
		}

		match entry.set_password(trimmed) {
			Ok(()) => {
				store.delete(key);
				dirty = true;
			}
			Err(e) => {
				log::warn!("Failed migrating API key to secure storage ({}): {}", key, e);
			}
		}
	}

	if dirty {
		let _ = store.save();
	}

	Ok(())
}

// ---------------------------------------------------------------------------
// Non-desktop stubs
// ---------------------------------------------------------------------------

#[cfg(not(desktop))]
pub fn has_api_key(_app: &tauri::AppHandle, _store_key: &str) -> bool {
	false
}

#[cfg(not(desktop))]
pub fn get_api_key(_app: &tauri::AppHandle, _store_key: &str) -> Option<String> {
	None
}

#[cfg(not(desktop))]
pub fn set_api_key(
	_app: &tauri::AppHandle,
	_store_key: &str,
	_api_key: &str,
) -> Result<(), String> {
	Ok(())
}

#[cfg(not(desktop))]
pub fn clear_api_key(_app: &tauri::AppHandle, _store_key: &str) -> Result<(), String> {
	Ok(())
}

#[cfg(not(desktop))]
pub fn migrate_api_keys_from_store(
	_app: &tauri::AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
	Ok(())
}
