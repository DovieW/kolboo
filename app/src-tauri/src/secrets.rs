//! Secure storage for secrets (API keys).
//!
//! Goal: keep API keys out of `settings.json` (plaintext at rest).
//! We store secrets in the OS keychain/credential manager when available,
//! and fall back to the store for legacy installs during migration.

#[cfg(desktop)]
use std::error::Error;
#[cfg(desktop)]
use std::sync::{Mutex, MutexGuard};

#[cfg(desktop)]
use tauri::AppHandle;

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

#[cfg(desktop)]
use crate::settings::store::get_fresh_settings_store;

#[cfg(desktop)]
use keyring::Entry;

#[cfg(all(desktop, target_os = "linux"))]
use std::collections::BTreeSet;

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

// Secret Service and some platform keyring backends do not reliably tolerate
// concurrent operations from one process. Commands run these blocking calls on
// worker threads, while this lock keeps the backend access itself serialized.
#[cfg(desktop)]
static SECRET_STORE_LOCK: Mutex<()> = Mutex::new(());

#[cfg(desktop)]
fn lock_secret_store() -> MutexGuard<'static, ()> {
    SECRET_STORE_LOCK.lock().unwrap_or_else(|poisoned| {
        log::warn!("Recovering a poisoned secure-storage lock");
        poisoned.into_inner()
    })
}

#[cfg(desktop)]
pub const AUTH_SESSION_ACCESS_TOKEN_KEY: &str = "license_access_token";

#[cfg(desktop)]
pub const AUTH_SESSION_REFRESH_TOKEN_KEY: &str = "license_refresh_token";

#[cfg(desktop)]
const EXTRA_SECRET_KEYS: &[&str] = &[
    "github_gist_token",
    AUTH_SESSION_ACCESS_TOKEN_KEY,
    AUTH_SESSION_REFRESH_TOKEN_KEY,
];

#[cfg(desktop)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSessionMaterial {
    pub access_token: String,
    pub refresh_token: String,
}

#[cfg(desktop)]
fn validate_secret_store_key(store_key: &str) -> Result<(), String> {
    let is_extra = EXTRA_SECRET_KEYS.contains(&store_key);
    if !store_key.ends_with("_api_key") && !is_extra {
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
    validate_secret_store_key(store_key)?;
    Entry::new(SERVICE_NAME, store_key).map_err(|e| e.to_string())
}

#[cfg(all(desktop, target_os = "linux"))]
fn legacy_linux_entry_for_key(store_key: &str) -> Result<Option<Entry>, String> {
    validate_secret_store_key(store_key)?;
    let credential =
        match keyring::keyutils::KeyutilsCredential::new_with_target(None, SERVICE_NAME, store_key)
        {
            Ok(credential) => credential,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
    Ok(Some(Entry::new_with_credential(Box::new(credential))))
}

#[cfg(all(desktop, target_os = "linux"))]
fn non_empty_password(entry: &Entry) -> Result<Option<String>, keyring::Error> {
    match entry.get_password() {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(all(desktop, target_os = "linux"))]
fn migration_api_key_names(app: &AppHandle) -> BTreeSet<String> {
    let mut keys = API_KEY_SETTING_KEYS
        .iter()
        .map(|key| (*key).to_string())
        .collect::<BTreeSet<_>>();
    keys.extend(
        crate::custom_providers::load(app)
            .into_iter()
            .map(|provider| provider.key_name()),
    );
    keys
}

/// Move credentials written by older Linux builds from the session-only kernel
/// keyring into Secret Service. The old value is deleted only after a matching
/// persistent readback succeeds.
#[cfg(all(desktop, target_os = "linux"))]
pub fn migrate_linux_session_keyring_secrets(app: &AppHandle) {
    for key in migration_api_key_names(app)
        .into_iter()
        .chain(["github_gist_token".to_string()])
    {
        if let Err(error) = migrate_linux_session_secret(&key) {
            log::warn!(
                "Could not migrate {} from the legacy Linux session keyring: {}",
                key,
                error
            );
        }
    }

    if let Err(error) = migrate_linux_auth_session() {
        log::warn!(
            "Could not migrate the auth session from the legacy Linux session keyring: {}",
            error
        );
    }
}

#[cfg(all(desktop, target_os = "linux"))]
fn migrate_linux_session_secret(store_key: &str) -> Result<(), String> {
    let persistent = entry_for_key(store_key)?;

    if non_empty_password(&persistent)
        .map_err(|e| e.to_string())?
        .is_some()
    {
        return Ok(());
    }

    let Some(legacy) = legacy_linux_entry_for_key(store_key)? else {
        return Ok(());
    };
    let Some(value) = non_empty_password(&legacy).map_err(|e| e.to_string())? else {
        return Ok(());
    };

    persistent.set_password(&value).map_err(|e| e.to_string())?;
    let verified = non_empty_password(&persistent).map_err(|e| e.to_string())?;
    if verified.as_deref() != Some(value.as_str()) {
        return Err("persistent secure-storage readback did not match".to_string());
    }

    match legacy.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => {
            log::warn!(
                "Migrated {}, but could not remove its obsolete Linux session-keyring copy: {}",
                store_key,
                error
            );
            Ok(())
        }
    }
}

#[cfg(all(desktop, target_os = "linux"))]
fn migrate_linux_auth_session() -> Result<(), String> {
    let persistent_access = entry_for_key(AUTH_SESSION_ACCESS_TOKEN_KEY)?;
    let persistent_refresh = entry_for_key(AUTH_SESSION_REFRESH_TOKEN_KEY)?;

    let current_access = non_empty_password(&persistent_access).map_err(|e| e.to_string())?;
    let current_refresh = non_empty_password(&persistent_refresh).map_err(|e| e.to_string())?;
    if current_access.is_some() && current_refresh.is_some() {
        return Ok(());
    }

    let Some(legacy_access) = legacy_linux_entry_for_key(AUTH_SESSION_ACCESS_TOKEN_KEY)? else {
        return Ok(());
    };
    let Some(legacy_refresh) = legacy_linux_entry_for_key(AUTH_SESSION_REFRESH_TOKEN_KEY)? else {
        return Ok(());
    };
    let old_access = non_empty_password(&legacy_access).map_err(|e| e.to_string())?;
    let old_refresh = non_empty_password(&legacy_refresh).map_err(|e| e.to_string())?;

    let desired_access = current_access.clone().or(old_access);
    let desired_refresh = current_refresh.clone().or(old_refresh);
    let (Some(desired_access), Some(desired_refresh)) = (desired_access, desired_refresh) else {
        // Never turn an incomplete legacy session into a persistent partial session.
        return Ok(());
    };

    let wrote_access = current_access.is_none();
    let wrote_refresh = current_refresh.is_none();
    let migration = (|| -> Result<(), String> {
        if wrote_access {
            persistent_access
                .set_password(&desired_access)
                .map_err(|e| e.to_string())?;
        }
        if wrote_refresh {
            persistent_refresh
                .set_password(&desired_refresh)
                .map_err(|e| e.to_string())?;
        }

        let verified_access = non_empty_password(&persistent_access).map_err(|e| e.to_string())?;
        let verified_refresh =
            non_empty_password(&persistent_refresh).map_err(|e| e.to_string())?;
        if verified_access.as_deref() != Some(desired_access.as_str())
            || verified_refresh.as_deref() != Some(desired_refresh.as_str())
        {
            return Err("persistent auth-session readback did not match".to_string());
        }
        Ok(())
    })();

    if let Err(error) = migration {
        if wrote_access {
            let _ = persistent_access.delete_credential();
        }
        if wrote_refresh {
            let _ = persistent_refresh.delete_credential();
        }
        return Err(error);
    }

    for legacy in [&legacy_access, &legacy_refresh] {
        match legacy.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => log::warn!(
                "Migrated the auth session, but could not remove an obsolete Linux session-keyring copy: {}",
                error
            ),
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Generic secret helpers (OS keyring)
// ---------------------------------------------------------------------------

/// Get a non-API-key secret from secure storage.
///
/// Unlike API keys, these do not have a legacy `settings.json` fallback.
#[cfg(desktop)]
pub fn get_secret(app: &AppHandle, store_key: &str) -> Option<String> {
    let _ = app;
    let _guard = lock_secret_store();
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
        Err(keyring::Error::NoEntry) => None,
        Err(e) => {
            log::warn!(
                "Failed to read secret from secure storage ({}): {}",
                store_key,
                e
            );
            None
        }
    }
}

#[cfg(desktop)]
pub fn has_secret(app: &AppHandle, store_key: &str) -> bool {
    get_secret(app, store_key).is_some()
}

#[cfg(desktop)]
pub fn set_secret(app: &AppHandle, store_key: &str, value: &str) -> Result<(), String> {
    let _ = app;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("Secret cannot be empty".to_string());
    }
    let _guard = lock_secret_store();
    let entry = entry_for_key(store_key)?;
    entry.set_password(trimmed).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(desktop)]
fn resolve_delete_failure(
    delete_error: String,
    credential_readback: Result<Option<()>, String>,
) -> Result<(), String> {
    match credential_readback {
        // Some Secret Service implementations can report an error after the
        // item was already removed. Treat the verified end state as success so
        // logout and key removal remain idempotent.
        Ok(None) => Ok(()),
        Ok(Some(())) => Err(delete_error),
        Err(read_error) => Err(format!(
            "{delete_error}; secure storage cleanup could not be verified: {read_error}"
        )),
    }
}

#[cfg(desktop)]
pub fn clear_secret(app: &AppHandle, store_key: &str) -> Result<(), String> {
    let _ = app;
    let _guard = lock_secret_store();
    let entry = entry_for_key(store_key)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(delete_error) => {
            let delete_error = delete_error.to_string();
            let credential_readback = match entry.get_password() {
                Ok(_) => Ok(Some(())),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(read_error) => Err(read_error.to_string()),
            };

            let result = resolve_delete_failure(delete_error.clone(), credential_readback);
            if result.is_ok() {
                log::warn!(
                    "Secure storage reported a delete error for {}, but readback confirmed the credential is absent: {}",
                    store_key,
                    delete_error
                );
            }
            result
        }
    }
}

/// Persist desktop auth session material (access + refresh) in secure storage.
///
/// This helper keeps lifecycle behavior centralized and attempts rollback if one
/// credential write succeeds while the other fails.
#[cfg(desktop)]
pub fn persist_auth_session_material(
    app: &AppHandle,
    access_token: &str,
    refresh_token: &str,
) -> Result<(), String> {
    set_secret(app, AUTH_SESSION_ACCESS_TOKEN_KEY, access_token)?;
    if let Err(e) = set_secret(app, AUTH_SESSION_REFRESH_TOKEN_KEY, refresh_token) {
        if let Err(clear_err) = clear_secret(app, AUTH_SESSION_ACCESS_TOKEN_KEY) {
            log::warn!(
                "Auth session rollback failed after refresh write error: {}",
                clear_err
            );
        }
        return Err(e);
    }
    Ok(())
}

/// Load desktop auth session material from secure storage.
///
/// If one token is present and the other is missing, the helper clears both to
/// avoid leaving a partial/inconsistent session footprint.
#[cfg(desktop)]
pub fn load_auth_session_material(app: &AppHandle) -> Option<AuthSessionMaterial> {
    let access = get_secret(app, AUTH_SESSION_ACCESS_TOKEN_KEY);
    let refresh = get_secret(app, AUTH_SESSION_REFRESH_TOKEN_KEY);

    match (access, refresh) {
        (Some(access_token), Some(refresh_token)) => Some(AuthSessionMaterial {
            access_token,
            refresh_token,
        }),
        (None, None) => None,
        _ => {
            log::warn!(
                "Detected partial auth session material; clearing secure storage auth session keys"
            );
            let _ = clear_auth_session_material(app);
            None
        }
    }
}

#[cfg(desktop)]
pub fn clear_auth_session_material(app: &AppHandle) -> Result<(), String> {
    let mut errors: Vec<String> = Vec::new();

    if let Err(e) = clear_secret(app, AUTH_SESSION_ACCESS_TOKEN_KEY) {
        errors.push(e);
    }
    if let Err(e) = clear_secret(app, AUTH_SESSION_REFRESH_TOKEN_KEY) {
        errors.push(e);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// Best-effort read of a legacy API key from `settings.json`.
///
/// This is only for backward compatibility during migration.
#[cfg(desktop)]
fn get_legacy_api_key_from_store(app: &AppHandle, store_key: &str) -> Option<String> {
    let raw = get_fresh_settings_store(app)?.get(store_key)?;

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
    let _guard = lock_secret_store();
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
            log::warn!(
                "Failed to read API key from secure storage ({}): {}",
                store_key,
                e
            );
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

    let _guard = lock_secret_store();
    let entry = entry_for_key(store_key)?;
    entry.set_password(trimmed).map_err(|e| e.to_string())?;

    // Ensure plaintext keys are removed from the settings store.
    if let Some(store) = get_fresh_settings_store(app) {
        store.delete(store_key);
        let _ = store.save();
    }

    Ok(())
}

#[cfg(desktop)]
pub fn clear_api_key(app: &AppHandle, store_key: &str) -> Result<(), String> {
    clear_secret(app, store_key)?;

    // Also clear any legacy value that may remain.
    if let Some(store) = get_fresh_settings_store(app) {
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
        let Some(value) = store.get(key) else {
            continue;
        };
        let Ok(s) = serde_json::from_value::<String>(value) else {
            continue;
        };
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
                log::warn!(
                    "Failed migrating API key to secure storage ({}): {}",
                    key,
                    e
                );
            }
        }
    }

    if dirty {
        let _ = store.save();
    }

    Ok(())
}

#[cfg(all(test, desktop))]
mod tests {
    use super::resolve_delete_failure;

    #[test]
    fn delete_error_is_ignored_when_readback_confirms_absence() {
        assert_eq!(
            resolve_delete_failure("backend error".to_string(), Ok(None)),
            Ok(())
        );
    }

    #[test]
    fn delete_error_is_preserved_when_credential_remains() {
        assert_eq!(
            resolve_delete_failure("backend error".to_string(), Ok(Some(()))),
            Err("backend error".to_string())
        );
    }

    #[test]
    fn delete_error_includes_readback_failure_when_state_is_unknown() {
        assert_eq!(
            resolve_delete_failure(
                "backend error".to_string(),
                Err("readback error".to_string())
            ),
            Err(
                "backend error; secure storage cleanup could not be verified: readback error"
                    .to_string()
            )
        );
    }
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

#[cfg(not(desktop))]
pub fn persist_auth_session_material(
    _app: &tauri::AppHandle,
    _access_token: &str,
    _refresh_token: &str,
) -> Result<(), String> {
    Ok(())
}

#[cfg(not(desktop))]
pub fn clear_auth_session_material(_app: &tauri::AppHandle) -> Result<(), String> {
    Ok(())
}
