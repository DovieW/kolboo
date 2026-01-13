use crate::pipeline::SharedPipeline;
use crate::router_embeddings_cache;
use crate::settings::{IntentRouterStrategy, ProxySettings};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::collections::HashSet;
use std::time::Duration;
use tauri::{AppHandle, State};

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheRouterEmbeddingsResponse {
    pub provider: String,
    pub model: String,
    pub total_hints: usize,
    pub cached_now: usize,
    pub skipped_existing: usize,
    pub stored_inserted: usize,
    pub stored_updated: usize,
}

fn collect_candidate_hints(profile: &crate::llm::ProgramPromptProfile) -> Vec<(String, String)> {
    const DEFAULT_CANDIDATE_ID: &str = "__default__";

    let mut out: Vec<(String, String)> = Vec::new();

    for preset in &profile.presets {
        // If there are no hints, fall back to using the preset name as a weak hint.
        let mut hints: Vec<String> = Vec::new();
        for h in &preset.routing_hints {
            let t = h.trim();
            if !t.is_empty() {
                hints.push(t.to_string());
            }
        }
        if hints.is_empty() {
            let name = preset.name.trim();
            if name.is_empty() {
                hints.push(preset.id.trim().to_string());
            } else {
                hints.push(name.to_string());
            }
        }

        for hint in hints {
            out.push((preset.id.clone(), hint));
        }
    }

    if let Some(desc) = profile
        .default_preset_description
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        out.push((DEFAULT_CANDIDATE_ID.to_string(), desc.to_string()));
    }

    out
}

/// Precompute + persist embeddings for all preset hints used by embeddings-based routing.
///
/// This is a Settings helper invoked from the Router accordion.
#[cfg(desktop)]
#[tauri::command]
pub async fn cache_router_embeddings(
    app: AppHandle,
    pipeline: State<'_, SharedPipeline>,
    profile_id: String,
    force_refresh: Option<bool>,
) -> Result<CacheRouterEmbeddingsResponse, String> {
    let force_refresh = force_refresh.unwrap_or(false);

    // Resolve profile.
    let config = pipeline.config();
    let profile = config
        .llm_config
        .program_prompt_profiles
        .iter()
        .find(|p| p.id == profile_id)
        .cloned()
        .ok_or_else(|| format!("Unknown profile_id: {}", profile_id))?;

    let router = profile
        .router
        .as_ref()
        .ok_or_else(|| "Profile has no router settings".to_string())?;

    if !router.enabled || router.strategy != IntentRouterStrategy::Embeddings {
        return Err("Router must be enabled and set to embeddings".to_string());
    }

    let embedding_provider = router
        .embedding_provider
        .as_deref()
        .unwrap_or("openai")
        .to_string();
    if embedding_provider != "openai"
        && embedding_provider != "cohere"
        && embedding_provider != "fireworks"
    {
        return Err(format!(
            "Embeddings provider '{}' not supported",
            embedding_provider
        ));
    }

    let embedding_model_default = if embedding_provider == "cohere" {
        "embed-english-v3.0"
    } else if embedding_provider == "fireworks" {
        // Starter default: keep in sync with the UI model list.
        "fireworks/qwen3-embedding-0p6b"
    } else {
        "text-embedding-3-small"
    };
    let embedding_model = router
        .embedding_model
        .as_deref()
        .unwrap_or(embedding_model_default)
        .to_string();

    // Use provider key from the central LLM API keys map.
    let api_key = config
        .llm_api_keys
        .get(embedding_provider.as_str())
        .map(|s| s.as_str())
        .unwrap_or("");
    if api_key.trim().is_empty() {
        return Err(format!(
            "{} API key missing (required for embeddings router)",
            embedding_provider
        ));
    }

    let proxy_settings: ProxySettings = config.proxy_settings.clone();

    let client =
        crate::network::build_http_client_with_timeout(&proxy_settings, Duration::from_secs(30))
            .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let mut cached_now: usize = 0;
    let mut skipped_existing: usize = 0;

    let mut new_entries: HashMap<String, Vec<f32>> = HashMap::new();

    // If the store already contains embeddings, we can optionally preload them into memory
    // (useful if the app started before the store was populated, or if the runtime cache was cleared).
    let mut preload_from_store: HashMap<String, Vec<f32>> = HashMap::new();

    let (persisted_keys, persisted_map): (HashSet<String>, serde_json::Map<String, JsonValue>) =
        if force_refresh {
            (HashSet::new(), serde_json::Map::new())
        } else {
            let store = app
                .store(router_embeddings_cache::ROUTER_EMBEDDINGS_STORE_FILE)
                .map_err(|e| format!("Failed to get store: {e}"))?;
            match store.get(router_embeddings_cache::ROUTER_EMBEDDINGS_STORE_KEY) {
                Some(JsonValue::Object(map)) => {
                    let keys = map.keys().cloned().collect();
                    (keys, map)
                }
                _ => (HashSet::new(), serde_json::Map::new()),
            }
        };

    let hints = collect_candidate_hints(&profile);

    // Cohere rate limits can be quite strict (especially on free tiers). To avoid
    // sending one request per hint, batch Cohere embed calls.
    const COHERE_BATCH_SIZE: usize = 32;

    let mut cohere_pending: Vec<(String, String)> = Vec::new();
    for (_candidate_id, hint) in &hints {
        let hint = hint.trim();
        if hint.is_empty() {
            continue;
        }

        let cache_key = if embedding_provider == "cohere" {
            format!("cohere::{}::search_document::{}", embedding_model, hint)
        } else if embedding_provider == "fireworks" {
            format!("fireworks::{}::{}", embedding_model, hint)
        } else {
            // Back-compat: keep existing OpenAI cache key format.
            format!("openai::{}::{}", embedding_model, hint)
        };

        if !force_refresh {
            // If the embedding is already persisted, ensure it's also present in the in-memory cache.
            // (This keeps routing fast without requiring a restart.)
            if persisted_keys.contains(&cache_key) {
                if !pipeline.embedding_cache_contains_key(&cache_key) {
                    if let Some(raw) = persisted_map.get(&cache_key).and_then(|v| v.as_str()) {
                        if let Some(embedding) = router_embeddings_cache::decode_embedding_b64(raw)
                        {
                            preload_from_store.insert(cache_key.clone(), embedding);
                        }
                    }
                }
                skipped_existing += 1;
                continue;
            }

            // If the embedding exists in memory (e.g., from prior routing), persist it.
            if let Some(embedding) = pipeline.embedding_cache_get(&cache_key) {
                new_entries.insert(cache_key, embedding);
                cached_now += 1;
                continue;
            }
        }

        if embedding_provider == "cohere" {
            cohere_pending.push((cache_key, hint.to_string()));
            continue;
        }

        let embedding = if embedding_provider == "fireworks" {
            crate::embeddings::fireworks::embed_text(&client, api_key, &embedding_model, hint)
                .await
                .map_err(|e| format!("Embeddings request failed: {e}"))?
        } else {
            crate::embeddings::openai::embed_text(&client, api_key, &embedding_model, hint)
                .await
                .map_err(|e| format!("Embeddings request failed: {e}"))?
        };

        if !embedding.is_empty() {
            new_entries.insert(cache_key, embedding);
            cached_now += 1;
        }
    }

    if embedding_provider == "cohere" && !cohere_pending.is_empty() {
        for chunk in cohere_pending.chunks(COHERE_BATCH_SIZE) {
            let cache_keys: Vec<String> = chunk.iter().map(|(k, _)| k.clone()).collect();
            let texts: Vec<String> = chunk.iter().map(|(_, t)| t.clone()).collect();

            let embeddings = crate::embeddings::cohere::embed_texts(
                &client,
                api_key,
                &embedding_model,
                "search_document",
                &texts,
            )
            .await
            .map_err(|e| format!("Embeddings request failed: {e}"))?;

            for (cache_key, embedding) in cache_keys.into_iter().zip(embeddings.into_iter()) {
                if embedding.is_empty() {
                    continue;
                }
                new_entries.insert(cache_key, embedding);
                cached_now += 1;
            }
        }
    }

    // Preload any embeddings we found in the persisted store but were missing from memory.
    if !preload_from_store.is_empty() {
        pipeline.preload_embedding_cache(preload_from_store);
    }

    // Merge into in-memory cache.
    if !new_entries.is_empty() {
        pipeline.preload_embedding_cache(new_entries.clone());
    }

    let (stored_inserted, stored_updated) = if new_entries.is_empty() {
        (0, 0)
    } else {
        router_embeddings_cache::merge_router_embeddings_into_store(&app, &new_entries)?
    };

    Ok(CacheRouterEmbeddingsResponse {
        provider: embedding_provider,
        model: embedding_model,
        total_hints: hints.len(),
        cached_now,
        skipped_existing,
        stored_inserted,
        stored_updated,
    })
}

#[cfg(not(desktop))]
#[tauri::command]
pub async fn cache_router_embeddings(
    _app: AppHandle,
    _pipeline: State<'_, SharedPipeline>,
    _profile_id: String,
    _force_refresh: Option<bool>,
) -> Result<CacheRouterEmbeddingsResponse, String> {
    Err("Not supported on this platform".to_string())
}
