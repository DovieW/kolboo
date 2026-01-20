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
    fn set(&self, key: &str, value: Value);
    fn delete(&self, key: &str);
}

#[cfg(desktop)]
impl<R: Runtime> SettingsPatchStore for Store<R> {
    fn set(&self, key: &str, value: Value) {
        Store::set(self, key, value);
    }

    fn delete(&self, key: &str) {
        Store::delete(self, key);
    }
}

#[cfg(desktop)]
impl<R: Runtime> SettingsPatchStore for std::sync::Arc<Store<R>> {
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
    let mut payload: Map<String, Value> = Map::new();

    for (k, v) in patch {
        store.set(&k, v.clone());
        payload.insert(k, v);
    }

    for k in delete_keys {
        store.delete(&k);
        payload.insert(k, Value::Null);
    }

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
    }
}
