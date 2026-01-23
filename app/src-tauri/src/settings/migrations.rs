#[cfg(desktop)]
use serde_json::{json, Value};
#[cfg(desktop)]
use tauri::Runtime;
#[cfg(desktop)]
use tauri_plugin_store::Store;

#[cfg(desktop)]
pub(crate) const SETTINGS_VERSION_LATEST: u32 = 4;

// Version history:
// 1 -> 2: quick ask hotkey key rename, retention key split, enum typo fixes, auto_mute_audio -> playing_audio_handling
// 2 -> 3: cleanup_prompt_sections schema normalization (global + per-profile)
// 3 -> 4: rewrite profile rewrite_llm_enabled normalization (ensure explicit boolean for non-default profiles)

#[cfg(desktop)]
pub(crate) trait SettingsStore {
    fn get(&self, key: &str) -> Option<Value>;
    fn set(&self, key: &str, value: Value);
}

#[cfg(desktop)]
impl<R: Runtime> SettingsStore for Store<R> {
    fn get(&self, key: &str) -> Option<Value> {
        Store::get(self, key)
    }

    fn set(&self, key: &str, value: Value) {
        Store::set(self, key, value);
    }
}

#[cfg(desktop)]
impl<R: Runtime> SettingsStore for std::sync::Arc<Store<R>> {
    fn get(&self, key: &str) -> Option<Value> {
        Store::get(self.as_ref(), key)
    }

    fn set(&self, key: &str, value: Value) {
        Store::set(self.as_ref(), key, value);
    }
}

#[cfg(desktop)]
fn normalize_settings_version(raw: Option<Value>) -> u32 {
    let parsed = match raw {
        Some(Value::Number(n)) => n.as_u64(),
        Some(Value::String(s)) => s.parse::<u64>().ok(),
        _ => None,
    };

    match parsed {
        Some(v) if v >= 1 => std::cmp::min(v, u32::MAX as u64) as u32,
        _ => 1,
    }
}

#[cfg(desktop)]
fn normalize_retention_days(value: &Value) -> Option<f64> {
    let raw = match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }?;

    if !raw.is_finite() {
        return None;
    }

    let clamped = raw.round().clamp(0.0, 36_500.0);
    Some(clamped)
}

#[cfg(desktop)]
fn normalize_cleanup_prompt_sections(value: &Value) -> Option<Value> {
    match value {
        Value::Null => None,
        Value::String(s) => Some(json!({"system": {"content": s }})),
        Value::Object(obj) => {
            let out = if let Some(system) = obj.get("system") {
                match system {
                    Value::Object(sys_obj) => {
                        let content = match sys_obj.get("content") {
                            Some(Value::String(s)) => Value::String(s.clone()),
                            Some(Value::Null) | None => Value::Null,
                            Some(_) => Value::Null,
                        };
                        json!({"system": {"content": content }})
                    }
                    Value::String(s) => json!({"system": {"content": s }}),
                    Value::Null => json!({"system": {"content": null }}),
                    _ => json!({"system": {"content": null }}),
                }
            } else if let Some(main) = obj.get("main") {
                match main {
                    Value::String(s) => json!({"system": {"content": s }}),
                    Value::Null => json!({"system": {"content": null }}),
                    _ => json!({"system": {"content": null }}),
                }
            } else {
                // Unknown/malformed legacy object; normalize to a minimal schema-compatible shape.
                json!({"system": {"content": null }})
            };

            // Avoid pointless rewrites when the value is already normalized.
            if &out == value {
                None
            } else {
                Some(out)
            }
        }
        _ => Some(json!({"system": {"content": null }})),
    }
}

#[cfg(desktop)]
fn migrate_v1_to_v2(store: &impl SettingsStore) -> bool {
    let mut dirty = false;

    if store.get("quick_ask_hold_hotkey").is_none() {
        if let Some(value) = store.get("quick_ask_hotkey") {
            if !value.is_null() {
                store.set("quick_ask_hold_hotkey", value);
                dirty = true;
            }
        }
    }

    let unit_missing = matches!(
        store.get("transcription_retention_unit"),
        None | Some(Value::Null)
    );
    let value_missing = matches!(
        store.get("transcription_retention_value"),
        None | Some(Value::Null)
    );
    if unit_missing && value_missing {
        if let Some(value) = store.get("transcription_retention_days") {
            if let Some(days) = normalize_retention_days(&value) {
                store.set("transcription_retention_unit", json!("days"));
                store.set("transcription_retention_value", json!(days));
                dirty = true;
            }
        }
    }

    if let Some(Value::String(value)) = store.get("overlay_monitor_target") {
        if value == "activeWindow" {
            store.set("overlay_monitor_target", json!("active_window"));
            dirty = true;
        }
    }

    let handling_missing = matches!(
        store.get("playing_audio_handling"),
        None | Some(Value::Null)
    );
    if handling_missing {
        if let Some(Value::Bool(value)) = store.get("auto_mute_audio") {
            store.set(
                "playing_audio_handling",
                json!(if value { "mute" } else { "none" }),
            );
            dirty = true;
        }
    }

    dirty
}

#[cfg(desktop)]
fn migrate_v2_to_v3(store: &impl SettingsStore) -> bool {
    let mut dirty = false;

    if let Some(value) = store.get("cleanup_prompt_sections") {
        if let Some(normalized) = normalize_cleanup_prompt_sections(&value) {
            store.set("cleanup_prompt_sections", normalized);
            dirty = true;
        }
    }

    if let Some(Value::Array(arr)) = store.get("rewrite_program_prompt_profiles") {
        let mut changed = false;
        let mut out = Vec::with_capacity(arr.len());

        for profile in arr {
            match profile {
                Value::Object(mut obj) => {
                    if let Some(raw) = obj.get("cleanup_prompt_sections") {
                        if let Some(normalized) = normalize_cleanup_prompt_sections(raw) {
                            obj.insert("cleanup_prompt_sections".to_string(), normalized);
                            changed = true;
                        }
                    }
                    out.push(Value::Object(obj));
                }
                other => out.push(other),
            }
        }

        if changed {
            store.set("rewrite_program_prompt_profiles", Value::Array(out));
            dirty = true;
        }
    }

    dirty
}

#[cfg(desktop)]
fn migrate_v3_to_v4(store: &impl SettingsStore) -> bool {
    let mut dirty = false;

    let default_rewrite_enabled = match store.get("rewrite_llm_enabled") {
        Some(Value::Bool(b)) => b,
        _ => false,
    };

    if let Some(Value::Array(arr)) = store.get("rewrite_program_prompt_profiles") {
        let mut changed = false;
        let mut out = Vec::with_capacity(arr.len());

        for profile in arr {
            match profile {
                Value::Object(mut obj) => {
                    let profile_id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("");

                    if profile_id != "default" {
                        let needs_fix =
                            !matches!(obj.get("rewrite_llm_enabled"), Some(Value::Bool(_)));

                        if needs_fix {
                            obj.insert(
                                "rewrite_llm_enabled".to_string(),
                                json!(default_rewrite_enabled),
                            );
                            changed = true;
                        }
                    }

                    out.push(Value::Object(obj));
                }
                other => out.push(other),
            }
        }

        if changed {
            store.set("rewrite_program_prompt_profiles", Value::Array(out));
            dirty = true;
        }
    }

    dirty
}

#[cfg(desktop)]
pub(crate) fn run_settings_migrations(
    store: &impl SettingsStore,
) -> Result<bool, Box<dyn std::error::Error>> {
    let current_version = normalize_settings_version(store.get("settings_version"));

    if current_version > SETTINGS_VERSION_LATEST {
        return Ok(false);
    }

    let mut dirty = false;
    let mut version = current_version;

    if version < 2 {
        dirty |= migrate_v1_to_v2(store);
        version = 2;
    }

    if version < 3 {
        dirty |= migrate_v2_to_v3(store);
        version = 3;
    }

    if version < 4 {
        dirty |= migrate_v3_to_v4(store);
        version = 4;
    }

    if version != current_version {
        store.set("settings_version", json!(version));
        dirty = true;
    }

    Ok(dirty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Default)]
    struct TestStore {
        values: RefCell<HashMap<String, Value>>,
    }

    impl TestStore {
        fn with_entries(entries: Vec<(&str, Value)>) -> Self {
            let mut values = HashMap::new();
            for (key, value) in entries {
                values.insert(key.to_string(), value);
            }
            Self {
                values: RefCell::new(values),
            }
        }
    }

    impl SettingsStore for TestStore {
        fn get(&self, key: &str) -> Option<Value> {
            self.values.borrow().get(key).cloned()
        }

        fn set(&self, key: &str, value: Value) {
            self.values.borrow_mut().insert(key.to_string(), value);
        }
    }

    #[test]
    fn migrates_quick_ask_hotkey_and_version() {
        let store = TestStore::with_entries(vec![(
            "quick_ask_hotkey",
            json!({"key":"F3","modifiers":[]}),
        )]);

        let dirty = run_settings_migrations(&store).expect("migration failed");

        assert!(dirty);
        assert!(store.get("quick_ask_hold_hotkey").is_some());
        assert_eq!(store.get("settings_version"), Some(json!(4)));
    }

    #[test]
    fn migrates_cleanup_prompt_sections_global_and_profile() {
        let store = TestStore::with_entries(vec![
            (
                "cleanup_prompt_sections",
                json!({"main": "Hello legacy prompt"}),
            ),
            (
                "rewrite_program_prompt_profiles",
                json!([
                    {"id": "default", "cleanup_prompt_sections": {"main": "P1"}},
                    {"id": "other", "cleanup_prompt_sections": null}
                ]),
            ),
            ("settings_version", json!(2)),
        ]);

        let dirty = run_settings_migrations(&store).expect("migration failed");
        assert!(dirty);
        assert_eq!(
            store.get("cleanup_prompt_sections"),
            Some(json!({"system": {"content": "Hello legacy prompt"}}))
        );

        let profiles = store.get("rewrite_program_prompt_profiles").unwrap();
        let arr = profiles.as_array().unwrap();
        let p1 = arr[0].as_object().unwrap();
        assert_eq!(
            p1.get("cleanup_prompt_sections"),
            Some(&json!({"system": {"content": "P1"}}))
        );

        assert_eq!(store.get("settings_version"), Some(json!(4)));
    }

    #[test]
    fn migrates_profile_rewrite_llm_enabled_to_explicit_bool() {
        let store = TestStore::with_entries(vec![
            ("settings_version", json!(3)),
            ("rewrite_llm_enabled", json!(true)),
            (
                "rewrite_program_prompt_profiles",
                json!([
                    {"id": "default", "rewrite_llm_enabled": null},
                    {"id": "chrome.exe"},
                    {"id": "code.exe", "rewrite_llm_enabled": false}
                ]),
            ),
        ]);

        let dirty = run_settings_migrations(&store).expect("migration failed");
        assert!(dirty);
        assert_eq!(store.get("settings_version"), Some(json!(4)));

        let profiles = store.get("rewrite_program_prompt_profiles").unwrap();
        let arr = profiles.as_array().unwrap();

        let default_profile = arr[0].as_object().unwrap();
        assert_eq!(
            default_profile.get("rewrite_llm_enabled"),
            Some(&Value::Null)
        );

        let chrome_profile = arr[1].as_object().unwrap();
        assert_eq!(
            chrome_profile.get("rewrite_llm_enabled"),
            Some(&json!(true))
        );

        let code_profile = arr[2].as_object().unwrap();
        assert_eq!(code_profile.get("rewrite_llm_enabled"), Some(&json!(false)));
    }

    #[test]
    fn migrates_transcription_retention_days() {
        let store = TestStore::with_entries(vec![("transcription_retention_days", json!(12))]);

        let dirty = run_settings_migrations(&store).expect("migration failed");

        assert!(dirty);
        assert_eq!(
            store.get("transcription_retention_unit"),
            Some(json!("days"))
        );
        assert_eq!(
            store.get("transcription_retention_value"),
            Some(json!(12.0))
        );
    }

    #[test]
    fn normalizes_overlay_monitor_target() {
        let store =
            TestStore::with_entries(vec![("overlay_monitor_target", json!("activeWindow"))]);

        let dirty = run_settings_migrations(&store).expect("migration failed");

        assert!(dirty);
        assert_eq!(
            store.get("overlay_monitor_target"),
            Some(json!("active_window"))
        );
    }

    #[test]
    fn migrates_auto_mute_audio() {
        let store = TestStore::with_entries(vec![("auto_mute_audio", json!(true))]);

        let dirty = run_settings_migrations(&store).expect("migration failed");

        assert!(dirty);
        assert_eq!(store.get("playing_audio_handling"), Some(json!("mute")));
    }
}
