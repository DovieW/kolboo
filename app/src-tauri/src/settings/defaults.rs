#[cfg(desktop)]
use crate::secrets;
#[cfg(desktop)]
use serde_json::{json, Value};
#[cfg(desktop)]
use tauri::AppHandle;
#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

#[cfg(desktop)]
use super::{default_definitions, default_values, migrations};

/// Ensure settings shown in the UI match what the backend will use.
///
/// The frontend often treats missing keys as "unset" and shows fallback defaults.
/// If the backend uses different fallbacks, this can cause confusing mismatches.
///
/// To prevent that, we eagerly seed `settings.json` with defaults for missing/null keys
/// (without overwriting any existing values).
#[cfg(desktop)]
pub(crate) fn ensure_default_settings(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let store = app.store("settings.json")?;

    let mut dirty = migrations::run_settings_migrations(&store)?;

    // Keep these defaults aligned with pipeline defaults / expected backend behavior.
    // We intentionally seed these so a brand new install has the same effective
    // settings that the pipeline will use at runtime (and what the UI shows).
    let default_pipeline_config = crate::pipeline::PipelineConfig::default();

    let is_missing = |v: Option<Value>| -> bool { matches!(v, None | Some(Value::Null)) };

    // Some settings intentionally use explicit null as a meaningful value.
    // For those keys, we only seed defaults when the key is truly absent.
    //
    // IMPORTANT: this closure must *not* capture `dirty`, otherwise `dirty` becomes
    // mutably borrowed for the lifetime of the closure and we can't update it
    // elsewhere (Rust E0506).
    let set_default = |key: &str, value: Value, only_if_absent: bool| -> bool {
        let should_set = if only_if_absent {
            store.get(key).is_none()
        } else {
            is_missing(store.get(key))
        };

        if should_set {
            store.set(key.to_string(), value);
            return true;
        }

        false
    };

    for definition in default_definitions::seedable_settings(&default_pipeline_config)? {
        dirty |= set_default(
            definition.key,
            definition.value,
            definition.seed_rule.only_if_absent(),
        );
    }

    // Rewrite profiles are partly static and partly a migration: historically this was
    // an empty array with the Default profile represented implicitly by global settings.
    // We now keep Default as a real persisted profile (id="default") so it can own
    // presets/router config, without discarding any user's existing profiles.
    let default_rewrite_profile = default_values::default_rewrite_profile_value();
    dirty |= set_default(
        "rewrite_program_prompt_profiles",
        json!([default_rewrite_profile.clone()]),
        false,
    );

    match store.get("rewrite_program_prompt_profiles") {
        Some(Value::Array(mut arr)) => {
            let has_default = arr.iter().any(|v| {
                v.as_object()
                    .and_then(|o| o.get("id"))
                    .and_then(|id| id.as_str())
                    .map(|id| id == "default")
                    .unwrap_or(false)
            });

            if !has_default {
                arr.insert(0, default_rewrite_profile);
                store.set(
                    "rewrite_program_prompt_profiles".to_string(),
                    Value::Array(arr),
                );
                dirty = true;
            }
        }
        Some(Value::Null) | None => {
            // Already handled by set_default above.
        }
        Some(_) => {
            // Malformed value: replace with a minimal sane default.
            store.set(
                "rewrite_program_prompt_profiles".to_string(),
                json!([default_rewrite_profile]),
            );
            dirty = true;
        }
    }

    if store.get("hotkey_shortcuts").is_none() {
        let mut cards: Vec<Value> = Vec::new();

        let push_card = |cards: &mut Vec<Value>, action: &str, value: Option<Value>| {
            let Some(Value::Object(_)) = value else {
                return;
            };

            cards.push(json!({
                "id": format!("seed-{}", action),
                "type": action,
                "hotkey": value,
            }));
        };

        push_card(cards.as_mut(), "toggle", store.get("toggle_hotkey"));
        push_card(cards.as_mut(), "hold", store.get("hold_hotkey"));
        push_card(cards.as_mut(), "paste_last", store.get("paste_last_hotkey"));
        push_card(cards.as_mut(), "retry", store.get("retry_hotkey"));

        let raw_quick_ask_hold = match store.get("quick_ask_hold_hotkey") {
            None => store.get("quick_ask_hotkey"),
            other => other,
        };
        push_card(cards.as_mut(), "quick_ask_hold", raw_quick_ask_hold);
        push_card(
            cards.as_mut(),
            "quick_ask_toggle",
            store.get("quick_ask_toggle_hotkey"),
        );

        store.set("hotkey_shortcuts".to_string(), Value::Array(cards));
        dirty = true;
    }

    if dirty {
        // Persist seeded defaults.
        // If saving fails, we don't want to crash the app; the runtime fallbacks will still work.
        if let Err(e) = store.save() {
            log::warn!("Failed to save seeded default settings: {}", e);
        }
    }

    // Best-effort: migrate legacy plaintext API keys out of `settings.json`.
    // This runs on startup (after the store exists), and deletes the store copy
    // only after the key was written to secure storage.
    let _ = secrets::migrate_api_keys_from_store(app);

    Ok(())
}
