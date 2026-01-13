use base64::{engine::general_purpose, Engine as _};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tauri::AppHandle;

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

pub const ROUTER_EMBEDDINGS_STORE_KEY: &str = "router_embeddings_cache_v1";
// IMPORTANT: Do not store large / frequently-mutated caches in settings.json.
// The store plugin maintains an in-memory map; saving from Rust can overwrite
// changes made by the JS side (and vice versa) if either side has a stale view.
// Using a dedicated store file avoids clobbering user settings like
// `hotkey_debug_enabled`.
#[cfg(desktop)]
pub const ROUTER_EMBEDDINGS_STORE_FILE: &str = "router_embeddings_cache.json";

/// Migration: older installs stored the embeddings cache in `settings.json`.
///
/// This cache can be very large, is not user-facing configuration, and can be
/// recreated. We move it into the dedicated cache store file and delete the
/// legacy key from `settings.json`.
#[cfg(desktop)]
pub fn migrate_router_embeddings_out_of_settings(app: &AppHandle) -> Result<usize, String> {
    use tauri_plugin_store::StoreExt;

    let settings_store = app
        .store("settings.json")
        .map_err(|e| format!("Failed to open settings store: {e}"))?;

    let Some(raw) = settings_store.get(ROUTER_EMBEDDINGS_STORE_KEY) else {
        return Ok(0);
    };

    let JsonValue::Object(map) = raw else {
        // Malformed: just delete it.
        settings_store.delete(ROUTER_EMBEDDINGS_STORE_KEY);
        let _ = settings_store.save();
        return Ok(0);
    };

    let mut decoded: HashMap<String, Vec<f32>> = HashMap::new();
    for (k, v) in map {
        let Some(b64) = v.as_str() else { continue };
        let Some(embedding) = decode_embedding_b64(b64) else {
            continue;
        };
        decoded.insert(k, embedding);
    }

    let migrated = decoded.len();
    if migrated > 0 {
        // Best-effort: persist to dedicated cache store.
        let _ = merge_router_embeddings_into_store(app, &decoded);
    }

    // Remove the legacy key regardless (don't keep giant blobs in settings).
    settings_store.delete(ROUTER_EMBEDDINGS_STORE_KEY);
    let _ = settings_store.save();

    Ok(migrated)
}

#[cfg(not(desktop))]
pub fn migrate_router_embeddings_out_of_settings(_app: &AppHandle) -> Result<usize, String> {
    Ok(0)
}

pub fn encode_embedding_b64(v: &[f32]) -> String {
    let mut bytes: Vec<u8> = Vec::with_capacity(v.len() * 4);
    for f in v {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
    general_purpose::STANDARD.encode(bytes)
}

pub fn decode_embedding_b64(s: &str) -> Option<Vec<f32>> {
    let bytes = general_purpose::STANDARD.decode(s).ok()?;
    if bytes.len() % 4 != 0 {
        return None;
    }

    let mut out: Vec<f32> = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        out.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[cfg(desktop)]
pub fn load_router_embeddings_from_store(app: &AppHandle) -> HashMap<String, Vec<f32>> {
    // Prefer the dedicated cache store.
    let raw = app
        .store(ROUTER_EMBEDDINGS_STORE_FILE)
        .ok()
        .and_then(|store| store.get(ROUTER_EMBEDDINGS_STORE_KEY))
        // Backward compatibility: older installs stored this cache in settings.json.
        .or_else(|| {
            app.store("settings.json")
                .ok()
                .and_then(|store| store.get(ROUTER_EMBEDDINGS_STORE_KEY))
        });

    let Some(raw) = raw else {
        return HashMap::new();
    };

    let JsonValue::Object(map) = raw else {
        return HashMap::new();
    };

    let mut out: HashMap<String, Vec<f32>> = HashMap::new();

    for (k, v) in map {
        let Some(b64) = v.as_str() else { continue };
        let Some(embedding) = decode_embedding_b64(b64) else {
            continue;
        };
        out.insert(k, embedding);
    }

    out
}

#[cfg(not(desktop))]
pub fn load_router_embeddings_from_store(_app: &AppHandle) -> HashMap<String, Vec<f32>> {
    HashMap::new()
}

#[cfg(desktop)]
pub fn merge_router_embeddings_into_store(
    app: &AppHandle,
    new_entries: &HashMap<String, Vec<f32>>,
) -> Result<(usize, usize), String> {
    let store = app
        .store(ROUTER_EMBEDDINGS_STORE_FILE)
        .map_err(|e| format!("Failed to get store: {e}"))?;

    let mut existing: serde_json::Map<String, JsonValue> = store
        .get(ROUTER_EMBEDDINGS_STORE_KEY)
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let mut inserted: usize = 0;
    let mut updated: usize = 0;

    for (k, v) in new_entries {
        let b64 = encode_embedding_b64(v);
        let next_value = JsonValue::String(b64);

        if existing.contains_key(k) {
            existing.insert(k.clone(), next_value);
            updated += 1;
        } else {
            existing.insert(k.clone(), next_value);
            inserted += 1;
        }
    }

    store.set(ROUTER_EMBEDDINGS_STORE_KEY, JsonValue::Object(existing));
    store
        .save()
        .map_err(|e| format!("Failed to save store: {e}"))?;

    Ok((inserted, updated))
}

#[cfg(not(desktop))]
pub fn merge_router_embeddings_into_store(
    _app: &AppHandle,
    _new_entries: &HashMap<String, Vec<f32>>,
) -> Result<(usize, usize), String> {
    Ok((0, 0))
}
