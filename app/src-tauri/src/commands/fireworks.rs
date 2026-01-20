//! Fireworks-specific helper commands.
//!
//! Primary use: list public model ids so the UI can offer a complete picker
//! without hardcoding Fireworks' rapidly-changing catalog.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

use crate::commands::CommandResult;
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelOption {
    pub value: String,
    pub label: String,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Clone)]
struct CachedModels {
    api_key_fingerprint: u64,
    fetched_at: Instant,
    models: Vec<ModelOption>,
}

static FIREWORKS_MODELS_CACHE: OnceLock<Mutex<Option<CachedModels>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FireworksModelsCacheFile {
    api_key_fingerprint: u64,
    fetched_at_unix_ms: u64,
    models: Vec<ModelOption>,
}

fn api_key_fingerprint(api_key: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    api_key.hash(&mut hasher);
    hasher.finish()
}

fn now_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64
}

fn fireworks_models_cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {e}"))?;
    let cache_dir = app_data_dir.join("cache");
    let _ = std::fs::create_dir_all(&cache_dir);
    Ok(cache_dir.join("fireworks-models.json"))
}

fn try_load_fireworks_models_disk_cache(
    app: &AppHandle,
    api_key_fp: u64,
    max_age: Duration,
) -> Option<Vec<ModelOption>> {
    let path = fireworks_models_cache_path(app).ok()?;
    let bytes = std::fs::read(&path).ok()?;
    let cache: FireworksModelsCacheFile = serde_json::from_slice(&bytes).ok()?;
    if cache.api_key_fingerprint != api_key_fp {
        return None;
    }
    let now = now_unix_ms();
    let age_ms = now.saturating_sub(cache.fetched_at_unix_ms);
    if age_ms > max_age.as_millis() as u64 {
        return None;
    }
    Some(cache.models)
}

fn persist_fireworks_models_disk_cache(app: &AppHandle, api_key_fp: u64, models: &[ModelOption]) {
    let Ok(path) = fireworks_models_cache_path(app) else {
        return;
    };
    let cache = FireworksModelsCacheFile {
        api_key_fingerprint: api_key_fp,
        fetched_at_unix_ms: now_unix_ms(),
        models: models.to_vec(),
    };
    if let Ok(json) = serde_json::to_vec(&cache) {
        let _ = std::fs::write(path, json);
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListModelsResponse {
    models: Vec<FireworksModel>,
    #[serde(default)]
    next_page_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAiModel {
    id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FireworksModel {
    name: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    public: Option<bool>,
}

fn normalize_label(model: &FireworksModel) -> String {
    if let Some(d) = model
        .display_name
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        return d.to_string();
    }

    // Fall back to the tail of the model id.
    model
        .name
        .rsplit('/')
        .next()
        .unwrap_or(model.name.as_str())
        .to_string()
}

fn normalize_label_from_id(id: &str) -> String {
    id.rsplit('/').next().unwrap_or(id).to_string()
}

fn is_probably_llm_model_id(name: &str) -> bool {
    let n = name.to_lowercase();
    if n.contains("/embedding") || n.contains("embedding") {
        return false;
    }
    if n.contains("whisper") {
        return false;
    }
    // Hide rerankers and other ranking models (not usable for chat rewrite).
    if n.contains("rerank") || n.contains("re-rank") || n.contains("reranker") {
        return false;
    }
    // Hide image generation / diffusion models.
    // Fireworks often exposes these in the same model catalog, but our UI picker here
    // is specifically for text rewrite LLMs.
    if n.contains("stable-diffusion")
        || n.contains("sdxl")
        || n.contains("diffusion")
        || n.contains("txt2img")
        || n.contains("text2img")
        || n.contains("t2i")
        || n.contains("image")
        || n.contains("flux")
        || n.contains("dall-e")
        || n.contains("dalle")
    {
        return false;
    }
    true
}

/// List public Fireworks models.
///
/// Preferred: use the OpenAI-compatible models endpoint (returns models the key can access):
/// `GET https://api.fireworks.ai/inference/v1/models`
///
/// Fallback: Fireworks REST API "List Models":
/// `GET https://api.fireworks.ai/v1/accounts/fireworks/models`
///
/// The returned list is filtered to *likely LLM/VLM* models (excludes embeddings
/// and whisper) to match UI use.
#[cfg(desktop)]
#[tauri::command]
pub async fn fireworks_list_models(app: AppHandle) -> CommandResult<Vec<ModelOption>> {
    let api_key: String =
        crate::secrets::get_api_key(&app, "fireworks_api_key").unwrap_or_default();
    let api_key_trimmed = api_key.trim();
    if api_key_trimmed.is_empty() {
        return Err("No Fireworks API key configured".to_string().into());
    }

    // Cache so Settings can re-render without repeatedly hitting Fireworks.
    // - Memory cache is short-lived for responsiveness during a session.
    // - Disk cache survives restarts to avoid repeated catalog fetches.
    // Both are keyed by a fingerprint of the API key (we never store the key itself).
    let mem_cache_ttl = Duration::from_secs(10 * 60);
    let disk_cache_ttl = Duration::from_secs(7 * 24 * 60 * 60);
    let fp = api_key_fingerprint(api_key_trimmed);
    let cache = FIREWORKS_MODELS_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some(cached) = guard.as_ref() {
            if cached.api_key_fingerprint == fp && cached.fetched_at.elapsed() <= mem_cache_ttl {
                return Ok(cached.models.clone());
            }
        }
    }

    if let Some(models) = try_load_fireworks_models_disk_cache(&app, fp, disk_cache_ttl) {
        if let Ok(mut guard) = cache.lock() {
            *guard = Some(CachedModels {
                api_key_fingerprint: fp,
                fetched_at: Instant::now(),
                models: models.clone(),
            });
        }
        return Ok(models);
    }

    let client = reqwest::Client::new();
    let mut out: Vec<ModelOption> = Vec::new();
    let mut callable_ids: Vec<String> = Vec::new();
    let mut catalog_models: Vec<FireworksModel> = Vec::new();

    // 1) Fetch OpenAI-compatible model discovery (models the key can actually call).
    // We still also fetch the catalog endpoint to:
    // - improve labels (display_name)
    // - show "catalog-only" models as disabled (prevents 404 surprises)
    {
        let resp = client
            .get("https://api.fireworks.ai/inference/v1/models")
            .header("Authorization", format!("Bearer {}", api_key_trimmed))
            .send()
            .await;

        if let Ok(resp) = resp {
            if resp.status().is_success() {
                let parsed: OpenAiModelsResponse = resp
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse Fireworks models response: {}", e))?;
                callable_ids = parsed
                    .data
                    .into_iter()
                    .map(|m| m.id)
                    .filter(|id| is_probably_llm_model_id(id))
                    .collect();
            }
        }
    }

    // 2) Fetch the Fireworks REST catalog endpoint (best-effort).
    let mut page_token: Option<String> = None;

    // Safety: cap pagination.
    for _ in 0..10 {
        let mut url = reqwest::Url::parse("https://api.fireworks.ai/v1/accounts/fireworks/models")
            .map_err(|e| e.to_string())?;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("pageSize", "200");
            if let Some(t) = page_token.as_deref() {
                qp.append_pair("pageToken", t);
            }
        }

        let resp = client
            .get(url)
            .header("Authorization", format!("Bearer {}", api_key_trimmed))
            .send()
            .await
            .map_err(|e| format!("Failed to list Fireworks models: {}", e))?;

        if !resp.status().is_success() {
            // Best-effort: if we already have callable models, don't fail just because
            // the catalog endpoint is unavailable.
            if !callable_ids.is_empty() {
                break;
            }
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Failed to list Fireworks models ({}): {}", status, body).into());
        }

        let parsed: ListModelsResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Fireworks models response: {}", e))?;

        catalog_models.extend(parsed.models.into_iter());

        page_token = parsed.next_page_token;
        if page_token
            .as_deref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        {
            break;
        }
    }

    let callable_set: HashSet<String> = callable_ids.into_iter().collect();
    let mut display_by_id: HashMap<String, String> = HashMap::new();

    for m in catalog_models.into_iter() {
        // Keep only public models if the flag exists.
        if let Some(is_public) = m.public {
            if !is_public {
                continue;
            }
        }

        // Best-effort filter: hide non-LLM ids.
        if !is_probably_llm_model_id(&m.name) {
            continue;
        }

        display_by_id
            .entry(m.name.clone())
            .or_insert_with(|| normalize_label(&m));
    }

    // Emit ONLY callable models (selectable). Catalog-only models are intentionally
    // omitted to avoid users selecting models that will 404 at inference time.
    let mut callable_out: Vec<ModelOption> = callable_set
        .iter()
        .map(|id| ModelOption {
            value: id.clone(),
            label: display_by_id
                .get(id)
                .cloned()
                .unwrap_or_else(|| normalize_label_from_id(id)),
            disabled: false,
        })
        .collect();
    callable_out.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));

    out.extend(callable_out);
    out.dedup_by(|a, b| a.value == b.value);

    if let Ok(mut guard) = cache.lock() {
        *guard = Some(CachedModels {
            api_key_fingerprint: fp,
            fetched_at: Instant::now(),
            models: out.clone(),
        });
    }
    persist_fireworks_models_disk_cache(&app, fp, &out);
    Ok(out)
}

/// Stub for non-desktop platforms.
#[cfg(not(desktop))]
#[tauri::command]
pub async fn fireworks_list_models(_app: AppHandle) -> CommandResult<Vec<ModelOption>> {
    Ok(Vec::new())
}
