//! Ollama-specific helper commands.
//!
//! Primary use: list locally available Ollama model ids so the UI can offer a
//! picker without requiring manual model entry.

use tauri::AppHandle;

use crate::app_shared;
use crate::commands::fireworks::ModelOption;

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

#[cfg(desktop)]
#[tauri::command]
pub async fn ollama_list_models(app: AppHandle) -> Result<Vec<ModelOption>, String> {
    let configured_url: Option<String> = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("ollama_url"))
        .and_then(|v| serde_json::from_value::<Option<String>>(v).ok())
        .and_then(app_shared::normalize_optional_base_url);

    let provider = crate::llm::OllamaLlmProvider::with_url(
        configured_url.unwrap_or_else(|| "http://localhost:11434".to_string()),
        None,
    );

    let mut models = provider
        .list_models()
        .await
        .map_err(|e| format!("Failed to list Ollama models: {e}"))?
        .into_iter()
        .map(|id| ModelOption {
            value: id.clone(),
            label: id,
            disabled: false,
        })
        .collect::<Vec<_>>();

    models.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
    Ok(models)
}

/// Stub for non-desktop platforms.
#[cfg(not(desktop))]
#[tauri::command]
pub async fn ollama_list_models(_app: AppHandle) -> Result<Vec<ModelOption>, String> {
    Ok(Vec::new())
}
