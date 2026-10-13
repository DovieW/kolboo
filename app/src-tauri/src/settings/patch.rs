use crate::commands::CommandError;
use serde_json::{Map, Value};

pub(crate) static WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(desktop)]
use tauri::Runtime;
#[cfg(desktop)]
use tauri_plugin_store::Store;

/// Abstraction for applying patches to the settings store.
///
/// We keep this as a small trait so we can unit test patch behavior without a real Tauri runtime.
pub trait SettingsPatchStore {
    fn get(&self, key: &str) -> Option<Value>;
    fn set(&self, key: &str, value: Value);
    fn delete(&self, key: &str);
}

/// Imported profiles replace their sidecars. Imported legacy model fields
/// must not retain newer atomic routes that override the restored selection.
pub(crate) fn model_import_delete_keys(
    incoming: &Map<String, Value>,
    existing_keys: impl IntoIterator<Item = String>,
) -> Vec<String> {
    existing_keys
        .into_iter()
        .filter(|key| {
            if incoming.contains_key(key) {
                return false;
            }
            if key.starts_with("model_selection_scope:") {
                return incoming.contains_key("rewrite_program_prompt_profiles");
            }
            let Some(action) = crate::model_selection::ModelAction::ALL
                .into_iter()
                .find(|action| action.settings_key() == key)
            else {
                return false;
            };
            let paths = match action {
                crate::model_selection::ModelAction::Transcription => ["stt_provider", "stt_model"],
                crate::model_selection::ModelAction::QuickAsk => {
                    ["quick_ask_provider", "quick_ask_model"]
                }
                crate::model_selection::ModelAction::QuickReplace => {
                    ["quick_replace_provider", "quick_replace_model"]
                }
                _ => ["llm_provider", "llm_model"],
            };
            paths.iter().any(|path| incoming.contains_key(*path))
        })
        .collect()
}

#[cfg(desktop)]
impl<R: Runtime> SettingsPatchStore for Store<R> {
    fn get(&self, key: &str) -> Option<Value> {
        Store::get(self, key)
    }

    fn set(&self, key: &str, value: Value) {
        Store::set(self, key, value);
    }

    fn delete(&self, key: &str) {
        Store::delete(self, key);
    }
}

#[cfg(desktop)]
impl<R: Runtime> SettingsPatchStore for std::sync::Arc<Store<R>> {
    fn get(&self, key: &str) -> Option<Value> {
        Store::get(self.as_ref(), key)
    }

    fn set(&self, key: &str, value: Value) {
        Store::set(self.as_ref(), key, value);
    }

    fn delete(&self, key: &str) {
        Store::delete(self.as_ref(), key);
    }
}

/// Apply a settings patch (set + delete) and return an event payload containing the changes.
///
/// - `patch`: keys to set to a JSON value
/// - `delete_keys`: keys to delete from the store
///
/// The returned map can be emitted as `settings-changed` so other windows can update.
/// For deleted keys, the payload contains the key with a `null` value.
pub fn apply_settings_patch(
    store: &dyn SettingsPatchStore,
    mut patch: Map<String, Value>,
    delete_keys: Vec<String>,
) -> Result<Map<String, Value>, CommandError> {
    const SETTINGS_REVISION_KEY: &str = "settings_revision";

    let mut scoped = false;
    for (key, value) in &patch {
        if key.starts_with("model_selection_scope:") {
            let (profile_id, preset_id, _) =
                crate::model_selection::parse_scoped_selection_key(key)
                    .map_err(|error| CommandError::new(error, "ValidationError"))?;
            let profiles = patch
                .get("rewrite_program_prompt_profiles")
                .cloned()
                .or_else(|| store.get("rewrite_program_prompt_profiles"));
            let exists = profiles
                .as_ref()
                .and_then(Value::as_array)
                .is_some_and(|profiles| {
                    profiles.iter().any(|profile| {
                        profile_id
                            .as_ref()
                            .is_none_or(|id| profile.get("id").and_then(Value::as_str) == Some(id))
                            && preset_id.as_ref().is_none_or(|id| {
                                profile
                                    .get("presets")
                                    .and_then(Value::as_array)
                                    .is_some_and(|presets| {
                                        presets.iter().any(|preset| {
                                            preset.get("id").and_then(Value::as_str) == Some(id)
                                        })
                                    })
                            })
                    })
                });
            if !exists {
                return Err(CommandError::new(
                    "This model scope no longer exists",
                    "ValidationError",
                ));
            }
            if !value.is_null() {
                let choice: crate::model_selection::ModelSelection =
                    serde_json::from_value(value.clone()).map_err(|_| {
                        CommandError::new("Invalid scoped model selection", "ValidationError")
                    })?;
                choice
                    .validate()
                    .map_err(|error| CommandError::new(error, "ValidationError"))?;
            }
            scoped = true;
        }
    }
    if scoped {
        patch.insert("model_selection_scopes_enabled".into(), Value::Bool(true));
    }

    for action in crate::model_selection::ModelAction::ALL {
        if let Some(value) = patch
            .get(action.settings_key())
            .filter(|value| !value.is_null())
        {
            let selection: crate::model_selection::ModelSelection =
                serde_json::from_value(value.clone())
                    .map_err(|_| CommandError::new("Invalid model selection", "ValidationError"))?;
            selection
                .validate()
                .map_err(|error| CommandError::new(error, "ValidationError"))?;
        }
    }
    if let Some(value) = patch
        .get("rewrite_program_prompt_profiles")
        .filter(|value| !value.is_null())
    {
        let profiles: Vec<crate::settings::RewriteProgramPromptProfile> =
            serde_json::from_value(value.clone())
                .map_err(|_| CommandError::new("Invalid profile settings", "ValidationError"))?;
        crate::model_selection::validate_profile_selections(&profiles)
            .map_err(|error| CommandError::new(error, "ValidationError"))?;
    }

    if scoped
        || patch.contains_key("stt_input_mode")
        || patch.contains_key("rewrite_program_prompt_profiles")
        || crate::model_selection::ModelAction::ALL
            .iter()
            .any(|action| patch.contains_key(action.settings_key()))
    {
        let projected = crate::model_selection::load_global_selections(&|key| {
            if delete_keys.iter().any(|deleted| deleted == key) {
                None
            } else {
                patch.get(key).cloned().or_else(|| store.get(key))
            }
        });
        if let Some(error) = projected.error {
            return Err(CommandError::new(error, "ValidationError"));
        }
    }

    let mut payload: Map<String, Value> = Map::new();
    super::managed_defaults::relinquish_on_edit(store, &patch, &delete_keys);

    for (k, v) in patch {
        store.set(&k, v.clone());
        payload.insert(k, v);
    }

    for k in delete_keys {
        store.delete(&k);
        payload.insert(k, Value::Null);
    }
    if scoped {
        payload.insert("rewrite_program_prompt_profiles".into(), Value::Bool(true));
    }

    // Monotonically increasing revision to help secondary windows ignore stale updates.
    // If the stored value is missing/invalid, treat it as 0.
    let current_revision = store
        .get(SETTINGS_REVISION_KEY)
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let next_revision = current_revision.saturating_add(1);
    store.set(SETTINGS_REVISION_KEY, Value::Number(next_revision.into()));
    payload.insert(
        SETTINGS_REVISION_KEY.to_string(),
        Value::Number(next_revision.into()),
    );

    Ok(payload)
}

/// Persist the entire patch or restore its previous in-memory state. Snapshot
/// only touched keys (not secret storage), including indirect ownership changes.
pub fn persist_settings_patch(
    store: &dyn SettingsPatchStore,
    patch: Map<String, Value>,
    delete_keys: Vec<String>,
    save: impl FnOnce() -> Result<(), CommandError>,
) -> Result<Map<String, Value>, CommandError> {
    let keys: std::collections::BTreeSet<_> = patch
        .keys()
        .cloned()
        .chain(delete_keys.iter().cloned())
        .chain([
            "settings_revision".into(),
            "model_selection_scopes_enabled".into(),
            super::managed_defaults::PENDING.into(),
        ])
        .collect();
    let previous: Vec<_> = keys
        .into_iter()
        .map(|key| {
            let value = store.get(&key);
            (key, value)
        })
        .collect();
    let payload = apply_settings_patch(store, patch, delete_keys)?;
    if let Err(error) = save() {
        for (key, value) in previous {
            match value {
                Some(value) => store.set(&key, value),
                None => store.delete(&key),
            }
        }
        return Err(error);
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[test]
    fn import_reconciliation_removes_only_superseded_routes_not_partial_settings_or_secrets() {
        let scope = crate::model_selection::scoped_selection_key(
            "p",
            None,
            crate::model_selection::ModelAction::Rewrite,
        );
        let keys = vec![
            scope.clone(),
            "model_selection_rewrite".into(),
            "model_selection_prompt_tools".into(),
            "model_selection_transcription".into(),
            "model_selection_quick_ask".into(),
            "model_selection_quick_replace".into(),
            "model_selection_intent_router".into(),
            "model_selection_history_analysis".into(),
            "provider_api_key".into(),
            "other".into(),
        ];
        assert!(model_import_delete_keys(&Map::new(), keys.clone()).is_empty());
        let incoming=serde_json::json!({"rewrite_program_prompt_profiles":[],"llm_provider":"legacy","stt_provider":"legacy","quick_ask_model":"legacy","quick_replace_model":"legacy"}).as_object().unwrap().clone();
        let removed = model_import_delete_keys(&incoming, keys.clone());
        assert_eq!(removed.len(), 8);
        assert!(
            !removed.contains(&"provider_api_key".into()) && !removed.contains(&"other".into())
        );
        let incoming=serde_json::json!({scope.clone():null,"rewrite_program_prompt_profiles":null,"llm_model":"legacy","model_selection_rewrite":null}).as_object().unwrap().clone();
        let removed = model_import_delete_keys(&incoming, keys);
        assert!(!removed.contains(&scope) && !removed.contains(&"model_selection_rewrite".into()));
        assert!(removed.contains(&"model_selection_prompt_tools".into()));
    }

    #[derive(Default)]
    struct FakeStore {
        data: RefCell<HashMap<String, Value>>,
    }

    impl SettingsPatchStore for FakeStore {
        fn get(&self, key: &str) -> Option<Value> {
            self.data.borrow().get(key).cloned()
        }

        fn set(&self, key: &str, value: Value) {
            self.data.borrow_mut().insert(key.to_string(), value);
        }

        fn delete(&self, key: &str) {
            self.data.borrow_mut().remove(key);
        }
    }

    #[test]
    fn apply_settings_patch_sets_and_deletes_and_returns_payload() {
        let store = FakeStore::default();

        let mut patch = Map::new();
        patch.insert("a".to_string(), Value::String("x".to_string()));
        patch.insert("b".to_string(), Value::Number(123.into()));

        let payload = apply_settings_patch(&store, patch, vec!["c".to_string()]).unwrap();

        let data = store.data.borrow();
        assert_eq!(data.get("a"), Some(&Value::String("x".to_string())));
        assert_eq!(data.get("b"), Some(&Value::Number(123.into())));
        assert!(data.get("c").is_none());

        assert_eq!(payload.get("a"), Some(&Value::String("x".to_string())));
        assert_eq!(payload.get("b"), Some(&Value::Number(123.into())));
        assert_eq!(payload.get("c"), Some(&Value::Null));

        let rev = payload
            .get("settings_revision")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        assert!(rev >= 1);
    }

    #[test]
    fn scoped_choice_is_independent_of_profile_metadata_and_persist_failure_rolls_back_all_indirect_keys(
    ) {
        let store = FakeStore::default();
        store.set(
            "rewrite_program_prompt_profiles",
            serde_json::json!([{"id":"p","name":"before","presets":[{"id":"q","name":"q"}]}]),
        );
        store.set("stt_provider", serde_json::json!("legacy"));
        store.set(
            super::super::managed_defaults::PENDING,
            serde_json::json!(true),
        );
        store.set("settings_revision", serde_json::json!(19));
        let before = store.data.borrow().clone();
        let key = crate::model_selection::scoped_selection_key(
            "p",
            Some("q"),
            crate::model_selection::ModelAction::Transcription,
        );
        let patch: Map<_, _> = [(
            key.clone(),
            serde_json::json!({"source":"byok","provider_id":"openai","model_id":"raw-id"}),
        )]
        .into_iter()
        .collect();
        let failed =
            persist_settings_patch(&store, patch.clone(), vec!["stt_provider".into()], || {
                Err(CommandError::unknown("disk unavailable"))
            });
        assert!(failed.is_err());
        assert_eq!(*store.data.borrow(), before);
        let payload = persist_settings_patch(&store, patch.clone(), vec![], || Ok(())).unwrap();
        assert_eq!(
            payload["rewrite_program_prompt_profiles"],
            Value::Bool(true)
        );
        assert_eq!(store.get(&key), patch.get(&key).cloned());
        assert_eq!(
            store.get("model_selection_scopes_enabled"),
            Some(Value::Bool(true))
        );
        assert_eq!(store.get("settings_revision"), Some(serde_json::json!(20)));
        assert_eq!(
            store.get(super::super::managed_defaults::PENDING),
            Some(Value::Bool(false))
        );
        assert_eq!(
            store.get("rewrite_program_prompt_profiles").unwrap()[0]["name"],
            "before"
        );
        apply_settings_patch(
            &store,
            [(
                "rewrite_program_prompt_profiles".into(),
                serde_json::json!([{"id":"p","name":"renamed","presets":[{"id":"q","name":"q"}]}]),
            )]
            .into_iter()
            .collect(),
            vec![],
        )
        .unwrap();
        let loaded = crate::model_selection::load_global_selections(&|key| store.get(key));
        assert_eq!(
            loaded.profiles[0].presets[0].model_selections
                [&crate::model_selection::ModelAction::Transcription]
                .model_id,
            "raw-id"
        );
    }

    #[test]
    fn malformed_and_missing_scopes_are_rejected_without_any_mutation() {
        let store = FakeStore::default();
        for (key, value) in [
            ("model_selection_scope:invalid", serde_json::json!(null)),
            (
                "model_selection_scope:[\"default\",null,\"rewrite\"]",
                serde_json::json!(null),
            ),
            (
                "model_selection_scope:[\"missing\",null,\"rewrite\"]",
                serde_json::json!(null),
            ),
            (
                "model_selection_rewrite",
                serde_json::json!({"source":"bad"}),
            ),
            (
                "model_selection_rewrite",
                serde_json::json!({"source":"byok","provider_id":"","model_id":"m"}),
            ),
            ("rewrite_program_prompt_profiles", serde_json::json!(false)),
            (
                "rewrite_program_prompt_profiles",
                serde_json::json!([{"id":"p","name":"p","model_selections":{"rewrite":{"source":"byok","provider_id":"p","model_id":""}}}]),
            ),
        ] {
            assert!(apply_settings_patch(
                &store,
                [(key.into(), value)].into_iter().collect(),
                vec![]
            )
            .is_err());
            assert!(store.data.borrow().is_empty());
        }
        store.set(
            "rewrite_program_prompt_profiles",
            serde_json::json!([{"id":"p","name":"p","presets":[{"id":"q","name":"q"}]}]),
        );
        let before = store.data.borrow().clone();
        for (preset, value) in [
            (None, serde_json::json!({"source":"bad"})),
            (
                Some("q"),
                serde_json::json!({"source":"byok","provider_id":"p","model_id":""}),
            ),
            (Some("missing"), Value::Null),
        ] {
            let key = crate::model_selection::scoped_selection_key(
                "p",
                preset,
                crate::model_selection::ModelAction::Rewrite,
            );
            assert!(
                apply_settings_patch(&store, [(key, value)].into_iter().collect(), vec![]).is_err()
            );
            assert_eq!(*store.data.borrow(), before);
        }
        store.set("rewrite_program_prompt_profiles",serde_json::json!([{"id":"p","name":"p","model_selections":{"rewrite":{"source":"byok","provider_id":"p","model_id":""}}}]));
        let key = crate::model_selection::scoped_selection_key(
            "p",
            None,
            crate::model_selection::ModelAction::Rewrite,
        );
        assert!(
            apply_settings_patch(&store, [(key, Value::Null)].into_iter().collect(), vec![])
                .is_ok()
        );
        // The explicit tombstone repairs the invalid route; it must not be revived.
        assert!(
            crate::model_selection::load_global_selections(&|key| store.get(key)).profiles[0]
                .model_selections
                .is_empty()
        );
    }

    #[test]
    fn conflicting_policy_projection_is_rejected_before_mutation() {
        let store = FakeStore::default();
        store.set("policy_state",serde_json::json!({"source":"cloud","is_valid":true,"enforced_fields":[{"path":"llm_provider"}]}));
        store.set(
            "policy_effective_values",
            serde_json::json!({"llm_provider":"enforced"}),
        );
        let before = store.data.borrow().clone();
        assert!(apply_settings_patch(&store,serde_json::json!({"model_selection_rewrite":{"source":"byok","provider_id":"different","model_id":"raw"}}).as_object().unwrap().clone(),vec![]).is_err());
        assert_eq!(*store.data.borrow(), before);
    }
}
