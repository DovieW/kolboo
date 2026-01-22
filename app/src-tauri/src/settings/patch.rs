use crate::commands::CommandError;
use serde_json::{Map, Value};

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
    store: &impl SettingsPatchStore,
    patch: Map<String, Value>,
    delete_keys: Vec<String>,
) -> Result<Map<String, Value>, CommandError> {
    const SETTINGS_REVISION_KEY: &str = "settings_revision";

    let mut payload: Map<String, Value> = Map::new();

    for (k, v) in patch {
        store.set(&k, v.clone());
        payload.insert(k, v);
    }

    for k in delete_keys {
        store.delete(&k);
        payload.insert(k, Value::Null);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

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
}
