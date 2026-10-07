//! First-install model defaults have explicit ownership. Never infer intent
//! from recording length or overwrite an existing BYOK configuration.
use super::patch::SettingsPatchStore;
use crate::managed_inference::ManagedModel;
use serde_json::{json, Map, Value};

pub(crate) const PENDING: &str = "managed_first_install_defaults_pending";
const CHOICES: &[&str] = &[
    "stt_provider",
    "stt_model",
    "llm_provider",
    "llm_model",
    "stt_use_managed_inference",
    "llm_use_managed_inference",
];

pub(crate) fn initialize(store: &impl SettingsPatchStore) -> bool {
    if store.get(PENDING).is_some() {
        return false;
    }
    let fresh = CHOICES.iter().all(|key| store.get(key).is_none());
    store.set(PENDING, json!(fresh));
    true
}

pub(crate) fn relinquish_on_edit(
    store: &impl SettingsPatchStore,
    patch: &Map<String, Value>,
    deleted: &[String],
) {
    if CHOICES
        .iter()
        .any(|key| patch.contains_key(*key) || deleted.iter().any(|item| item == key))
    {
        store.set(PENDING, json!(false));
    }
}

pub(crate) fn selection(
    store: &impl SettingsPatchStore,
    models: &[ManagedModel],
    has_saved_key: bool,
) -> Option<Map<String, Value>> {
    if store.get(PENDING).and_then(|value| value.as_bool()) != Some(true) || has_saved_key {
        return None;
    }
    let stt = published_default(models, "transcription")?;
    let llm = published_default(models, "chat_completions");
    let mut patch = json!({
        "stt_provider": stt.provider, "stt_model": stt.id, "stt_use_managed_inference": true
    })
    .as_object()
    .unwrap()
    .clone();
    if let Some(llm) = llm {
        patch.insert("llm_provider".into(), json!(llm.provider));
        patch.insert("llm_model".into(), json!(llm.id));
        patch.insert("llm_use_managed_inference".into(), json!(true));
    }
    patch.insert(PENDING.into(), json!(false));
    Some(patch)
}

fn published_default<'a>(models: &'a [ManagedModel], capability: &str) -> Option<&'a ManagedModel> {
    models.iter().find(|model| {
        model.default_for_provider && model.capabilities.iter().any(|value| value == capability)
    })
}

/// A failed disk write must leave the first-install marker and existing choices
/// intact, so a later verified entitlement refresh can retry model setup.
pub(crate) fn persist_selection(
    store: &impl SettingsPatchStore,
    patch: Map<String, Value>,
    save: impl FnOnce() -> Result<(), crate::commands::CommandError>,
) -> Result<Map<String, Value>, crate::commands::CommandError> {
    let previous: Vec<_> = patch
        .keys()
        .map(|key| (key.clone(), store.get(key)))
        .chain(std::iter::once((
            "settings_revision".into(),
            store.get("settings_revision"),
        )))
        .collect();
    let payload = super::patch::apply_settings_patch(store, patch, vec![])?;
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
    #[derive(Default)]
    struct Store(RefCell<Map<String, Value>>);
    impl SettingsPatchStore for Store {
        fn get(&self, key: &str) -> Option<Value> {
            self.0.borrow().get(key).cloned()
        }
        fn set(&self, key: &str, value: Value) {
            self.0.borrow_mut().insert(key.into(), value);
        }
        fn delete(&self, key: &str) {
            self.0.borrow_mut().remove(key);
        }
    }
    fn models() -> Vec<ManagedModel> {
        [
            ("transcription", "groq", "speech"),
            ("chat_completions", "openai", "text"),
        ]
        .into_iter()
        .map(|(capability, provider, id)| ManagedModel {
            id: id.into(),
            provider: provider.into(),
            display_name: id.into(),
            capabilities: vec![capability.into()],
            default_for_provider: true,
        })
        .collect()
    }
    #[test]
    fn fresh_install_uses_only_published_defaults_without_enabling_rewriting() {
        let store = Store::default();
        assert!(initialize(&store));
        assert!(!initialize(&store));
        let patch = selection(&store, &models(), false).unwrap();
        assert_eq!(patch.get("stt_model"), Some(&json!("speech")));
        assert_eq!(patch.get("llm_model"), Some(&json!("text")));
        assert_eq!(patch.get(PENDING), Some(&json!(false)));
        assert!(!patch.contains_key("rewrite_enabled"));
        assert!(selection(&store, &[], false).is_none());
        let speech_only = selection(&store, &models()[..1], false).unwrap();
        assert!(!speech_only.contains_key("llm_model"));
        let mut unpublished = models();
        unpublished[0].default_for_provider = false;
        assert!(selection(&store, &unpublished, false).is_none());
        assert!(selection(&store, &models(), true).is_none());
    }
    #[test]
    fn existing_explicit_choices_and_keys_are_preserved() {
        for key in CHOICES {
            let store = Store::default();
            store.set(key, Value::Null);
            initialize(&store);
            assert!(selection(&store, &models(), false).is_none());
        }
        let store = Store::default();
        initialize(&store);
        relinquish_on_edit(&store, &Map::new(), &[]);
        assert!(selection(&store, &models(), false).is_some());
        relinquish_on_edit(
            &store,
            &Map::from_iter([("stt_model".into(), json!("my-model"))]),
            &[],
        );
        assert!(selection(&store, &models(), false).is_none());
        store.set(PENDING, json!(true));
        relinquish_on_edit(&store, &Map::new(), &["stt_provider".into()]);
        assert!(selection(&store, &models(), false).is_none());
    }
    #[test]
    fn failed_save_preserves_choices_and_retry_marker_then_success_commits() {
        let store = Store::default();
        initialize(&store);
        store.set("stt_model", json!("previous"));
        let previous = store.0.borrow().clone();
        let patch = selection(&store, &models(), false).unwrap();
        let result = persist_selection(&store, patch, || {
            Err(crate::commands::CommandError::new(
                "disk unavailable",
                "settings",
            ))
        });
        assert!(result.is_err());
        assert_eq!(*store.0.borrow(), previous);
        store.set("settings_revision", json!(4));
        let patch = selection(&store, &models(), false).unwrap();
        assert_eq!(
            persist_selection(&store, patch, || Ok(())).unwrap()["settings_revision"],
            json!(5)
        );
        assert_eq!(store.get(PENDING), Some(json!(false)));
        assert_eq!(store.get("stt_model"), Some(json!("speech")));
        store.set(PENDING, json!(true));
        let previous = store.0.borrow().clone();
        let patch = selection(&store, &models(), false).unwrap();
        assert!(
            persist_selection(&store, patch, || Err(crate::commands::CommandError::new(
                "disk unavailable",
                "settings"
            )))
            .is_err()
        );
        assert_eq!(*store.0.borrow(), previous);
    }
}
