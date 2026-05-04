use crate::embeddings::{self, EmbeddingsProvider};
use crate::llm::LlmProvider;
use crate::settings::{IntentRouterStrategy, ProxySettings};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::AppHandle;

fn preview_for_log(s: &str, max_chars: usize) -> (String, bool, usize) {
    let len = s.chars().count();
    let mut preview: String = s.chars().take(max_chars).collect();
    let truncated = len > max_chars;
    if truncated {
        preview.push('…');
    }
    (preview, truncated, len)
}

struct RouterContext<'a> {
    embedding_provider: &'a str,
    embedding_model: &'a str,
    pick_highest_score: bool,
    threshold: f32,
    margin: f32,
}

struct RouterCallState {
    call_id: u64,
    calls_request: Vec<JsonValue>,
    calls_response: Vec<JsonValue>,
}

impl RouterCallState {
    fn new() -> Self {
        Self {
            call_id: 0,
            calls_request: Vec::new(),
            calls_response: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RoutingDecisionOutcome {
    SelectedPreset,
    DefaultTarget,
    NoDecision,
    Ambiguous,
    Failed,
    // Kept in the routing vocabulary even though current router adapters surface
    // cancellation through Transcription Flow; future cancellable adapters can use
    // the same strategy-independent outcome without changing callers.
    #[allow(dead_code)]
    Cancelled,
    UnknownPreset,
}

impl RoutingDecisionOutcome {
    fn as_str(&self) -> &'static str {
        match self {
            Self::SelectedPreset => "selected_preset",
            Self::DefaultTarget => "default_target",
            Self::NoDecision => "no_decision",
            Self::Ambiguous => "ambiguous",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::UnknownPreset => "unknown_preset",
        }
    }
}

fn router_response_with_outcome(
    mut response_json: JsonValue,
    outcome: &RoutingDecisionOutcome,
) -> JsonValue {
    if let Some(obj) = response_json.as_object_mut() {
        obj.insert(
            "outcome".to_string(),
            JsonValue::String(outcome.as_str().to_string()),
        );
    }
    response_json
}

#[derive(Debug, Clone)]
pub(super) struct RoutingDecision {
    pub selected_preset_id: Option<String>,
    pub outcome: RoutingDecisionOutcome,
    pub scores: Vec<(String, f32)>,
    pub threshold: Option<f32>,
    pub margin: Option<f32>,
    pub request_json: JsonValue,
    pub response_json: JsonValue,
}

impl RoutingDecision {
    fn embeddings(
        selected_preset_id: Option<String>,
        outcome: RoutingDecisionOutcome,
        scores: Vec<(String, f32)>,
        threshold: f32,
        margin: f32,
        request_json: JsonValue,
        response_json: JsonValue,
    ) -> Self {
        let response_json = router_response_with_outcome(response_json, &outcome);
        Self {
            selected_preset_id,
            outcome,
            scores,
            threshold: Some(threshold),
            margin: Some(margin),
            request_json,
            response_json,
        }
    }

    fn llm(
        selected_preset_id: Option<String>,
        outcome: RoutingDecisionOutcome,
        request_json: JsonValue,
        response_json: JsonValue,
    ) -> Self {
        let response_json = router_response_with_outcome(response_json, &outcome);
        Self {
            selected_preset_id,
            outcome,
            scores: Vec::new(),
            threshold: None,
            margin: None,
            request_json,
            response_json,
        }
    }
}

/// Route to a preset using embeddings-based similarity matching.
///
/// When `injected_provider` is `Some`, uses the injected provider for all embedding requests.
/// This is primarily for testing to enable deterministic, offline integration tests.
/// When `None`, creates real HTTP-based providers based on `llm_api_keys`.
pub(super) async fn route_preset_id_with_embeddings(
    profile: &crate::llm::ProgramPromptProfile,
    transcript: &str,
    proxy_settings: &ProxySettings,
    llm_api_keys: &HashMap<String, String>,
    embedding_cache: &Arc<Mutex<HashMap<String, Vec<f32>>>>,
    persist_app: Option<AppHandle>,
    injected_provider: Option<Arc<dyn EmbeddingsProvider>>,
) -> Option<RoutingDecision> {
    const DEFAULT_CANDIDATE_ID: &str = "__default__";

    let router = profile.router.as_ref()?;
    if !router.enabled || router.strategy != IntentRouterStrategy::Embeddings {
        return None;
    }

    let default_desc = profile
        .default_preset_description
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    // Routing only makes sense when there is more than one possible target.
    let candidate_count = profile.presets.len() + if default_desc.is_some() { 1 } else { 0 };
    if candidate_count < 2 {
        return None;
    }

    let embedding_provider = router.embedding_provider.as_deref().unwrap_or("openai");

    if embedding_provider != "openai"
        && embedding_provider != "cohere"
        && embedding_provider != "fireworks"
    {
        log::warn!(
            "Intent router: embeddings provider '{}' not supported; routing skipped",
            embedding_provider
        );
        return None;
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
        .unwrap_or(embedding_model_default);
    let pick_highest_score = router.pick_highest_score;
    let threshold = router.similarity_threshold.unwrap_or(0.78);
    let margin = router.similarity_margin.unwrap_or(0.05);

    // When an injected provider is present, we skip API key validation and HTTP client
    // creation since the provider handles everything internally (useful for testing).
    let api_key: String;
    let client: Option<reqwest::Client>;

    if injected_provider.is_some() {
        // Injected provider handles its own auth/network
        api_key = String::new();
        client = None;
    } else {
        let key = llm_api_keys
            .get(embedding_provider)
            .map(|s| s.as_str())
            .unwrap_or("");
        if key.trim().is_empty() {
            log::warn!(
                "Intent router: {} API key missing; embeddings routing skipped",
                embedding_provider
            );
            return None;
        }
        api_key = key.to_string();

        client = match crate::network::build_http_client_with_timeout(
            proxy_settings,
            Duration::from_secs(30),
        ) {
            Ok(c) => Some(c),
            Err(e) => {
                log::warn!("Intent router: failed to build HTTP client: {}", e);
                return None;
            }
        };
    }

    let transcript = transcript.trim();
    if transcript.is_empty() {
        return None;
    }

    let router_ctx = RouterContext {
        embedding_provider,
        embedding_model,
        pick_highest_score,
        threshold,
        margin,
    };
    let mut calls = RouterCallState::new();

    fn push_call(
        state: &mut RouterCallState,
        kind: &str,
        candidate_id: Option<&str>,
        from_cache: bool,
        request: JsonValue,
        response: JsonValue,
    ) {
        state.calls_request.push(serde_json::json!({
            "id": state.call_id,
            "kind": kind,
            "candidate_id": candidate_id,
            "from_cache": from_cache,
            "request": request,
        }));
        state.calls_response.push(serde_json::json!({
            "id": state.call_id,
            "kind": kind,
            "candidate_id": candidate_id,
            "from_cache": from_cache,
            "response": response,
        }));
        state.call_id += 1;
    }

    fn build_router_payloads(
        ctx: &RouterContext<'_>,
        selected: &Option<String>,
        scores: &[(String, f32)],
        calls: &RouterCallState,
    ) -> (JsonValue, JsonValue) {
        let router_request_json = serde_json::json!({
            "type": "embeddings",
            "provider": ctx.embedding_provider,
            "model": ctx.embedding_model,
            "pick_highest_score": ctx.pick_highest_score,
            "similarity_threshold": ctx.threshold,
            "similarity_margin": ctx.margin,
            "calls": calls.calls_request,
        });
        let router_response_json = serde_json::json!({
            "type": "embeddings",
            "selected_preset_id": selected,
            "scores": scores,
            "calls": calls.calls_response,
        });
        (router_request_json, router_response_json)
    }

    // Embed the transcript
    let transcript_input_type = if embedding_provider == "cohere" {
        Some("search_query")
    } else {
        None
    };

    let transcript_embedding_result: Result<(Vec<f32>, JsonValue, JsonValue), String> =
        if let Some(ref provider) = injected_provider {
            // Use injected provider (for testing)
            provider
                .embed_text(transcript, transcript_input_type)
                .await
                .map_err(|e| e.to_string())
        } else if let Some(ref http_client) = client {
            // Use real HTTP-based providers
            if embedding_provider == "cohere" {
                embeddings::cohere::embed_text_with_debug(
                    http_client,
                    &api_key,
                    embedding_model,
                    "search_query",
                    transcript,
                )
                .await
                .map_err(|e| e.to_string())
            } else if embedding_provider == "fireworks" {
                embeddings::fireworks::embed_text_with_debug(
                    http_client,
                    &api_key,
                    embedding_model,
                    transcript,
                )
                .await
                .map_err(|e| e.to_string())
            } else {
                embeddings::openai::embed_text_with_debug(
                    http_client,
                    &api_key,
                    embedding_model,
                    transcript,
                )
                .await
                .map_err(|e| e.to_string())
            }
        } else {
            Err("No embeddings provider available".to_string())
        };

    let transcript_embedding = match transcript_embedding_result {
        Ok((v, req, resp)) => {
            push_call(&mut calls, "transcript", None, false, req, resp);
            v
        }
        Err(e) => {
            log::warn!("Intent router: embeddings request failed: {}", e);

            let (preview, truncated, len) = preview_for_log(transcript, 800);
            let req = serde_json::json!({
                "provider": embedding_provider,
                "model": embedding_model,
                "input_type": if embedding_provider == "cohere" {
                    JsonValue::String("search_query".to_string())
                } else {
                    JsonValue::Null
                },
                "input_preview": preview,
                "input_len": len,
                "input_truncated": truncated,
            });

            let err_json = serde_json::from_str::<JsonValue>(&e).unwrap_or(JsonValue::String(e));
            let resp = serde_json::json!({ "error": err_json });
            push_call(&mut calls, "transcript", None, false, req, resp);

            let empty_scores: Vec<(String, f32)> = Vec::new();
            let (router_req, router_resp) =
                build_router_payloads(&router_ctx, &None, &empty_scores, &calls);
            return Some(RoutingDecision::embeddings(
                None,
                RoutingDecisionOutcome::Failed,
                empty_scores,
                threshold,
                margin,
                router_req,
                router_resp,
            ));
        }
    };

    // Compute a per-preset score as the best similarity across its hints.
    let mut best: Option<(String, f32)> = None;
    let mut second_best: Option<(String, f32)> = None;

    // Collect scores for diagnostics/UI.
    let mut scores: Vec<(String, f32)> = Vec::new();

    // Compute a per-candidate score as the best similarity across its hints.
    // Candidates include:
    // - each preset
    // - optionally, the implicit Default target (returns None)
    let mut candidates: Vec<(String, Vec<String>)> = Vec::new();
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

        candidates.push((preset.id.clone(), hints));
    }
    if let Some(desc) = default_desc {
        candidates.push((DEFAULT_CANDIDATE_ID.to_string(), vec![desc]));
    }

    for (candidate_id, hints) in candidates {
        let mut candidate_best: Option<f32> = None;

        for hint in hints {
            let cache_key = if embedding_provider == "cohere" {
                format!("cohere::{}::search_document::{}", embedding_model, hint)
            } else if embedding_provider == "fireworks" {
                format!("fireworks::{}::{}", embedding_model, hint)
            } else {
                // Back-compat: keep existing OpenAI cache key format.
                format!("openai::{}::{}", embedding_model, hint)
            };

            let cached_hint_embedding: Vec<f32> = {
                if let Ok(cache) = embedding_cache.lock() {
                    if let Some(v) = cache.get(&cache_key) {
                        v.clone()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                }
            };

            let hint_embedding = if !cached_hint_embedding.is_empty() {
                let (preview, truncated, len) = preview_for_log(&hint, 800);
                let req = serde_json::json!({
                    "provider": embedding_provider,
                    "model": embedding_model,
                    "input_type": if embedding_provider == "cohere" {
                        JsonValue::String("search_document".to_string())
                    } else {
                        JsonValue::Null
                    },
                    "cache_key": cache_key,
                    "input_preview": preview,
                    "input_len": len,
                    "input_truncated": truncated,
                });
                let resp = serde_json::json!({
                    "from_cache": true,
                    "embedding_len": cached_hint_embedding.len(),
                });
                push_call(
                    &mut calls,
                    "hint",
                    Some(candidate_id.as_str()),
                    true,
                    req,
                    resp,
                );
                cached_hint_embedding
            } else {
                // Embed the hint
                let hint_input_type = if embedding_provider == "cohere" {
                    Some("search_document")
                } else {
                    None
                };

                let embed_result: Result<(Vec<f32>, JsonValue, JsonValue), String> =
                    if let Some(ref provider) = injected_provider {
                        // Use injected provider (for testing)
                        provider
                            .embed_text(&hint, hint_input_type)
                            .await
                            .map_err(|e| e.to_string())
                    } else if let Some(ref http_client) = client {
                        // Use real HTTP-based providers
                        if embedding_provider == "cohere" {
                            embeddings::cohere::embed_text_with_debug(
                                http_client,
                                &api_key,
                                embedding_model,
                                "search_document",
                                &hint,
                            )
                            .await
                            .map_err(|e| e.to_string())
                        } else if embedding_provider == "fireworks" {
                            embeddings::fireworks::embed_text_with_debug(
                                http_client,
                                &api_key,
                                embedding_model,
                                &hint,
                            )
                            .await
                            .map_err(|e| e.to_string())
                        } else {
                            embeddings::openai::embed_text_with_debug(
                                http_client,
                                &api_key,
                                embedding_model,
                                &hint,
                            )
                            .await
                            .map_err(|e| e.to_string())
                        }
                    } else {
                        Err("No embeddings provider available".to_string())
                    };

                match embed_result {
                    Ok((v, req, resp)) => {
                        push_call(
                            &mut calls,
                            "hint",
                            Some(candidate_id.as_str()),
                            false,
                            req,
                            resp,
                        );
                        let cache_key_for_store = cache_key.clone();
                        if let Ok(mut cache) = embedding_cache.lock() {
                            // Keep cache bounded, but avoid aggressively clearing.
                            // The cache may be preloaded from persisted store and can be larger
                            // than a few hundred entries.
                            if cache.len() > 20_000 {
                                cache.clear();
                            }
                            cache.insert(cache_key, v.clone());
                        }

                        // Best-effort persistence: embeddings routing performance should improve
                        // automatically after first use without requiring a manual "Store" step.
                        if let Some(app) = persist_app.as_ref() {
                            let mut one: HashMap<String, Vec<f32>> = HashMap::new();
                            one.insert(cache_key_for_store, v.clone());
                            if let Err(e) =
                                crate::router_embeddings_cache::merge_router_embeddings_into_store(
                                    app, &one,
                                )
                            {
                                log::debug!(
                                    "Intent router: failed to persist router embeddings cache: {}",
                                    e
                                );
                            }
                        }
                        v
                    }
                    Err(e) => {
                        let (preview, truncated, len) = preview_for_log(&hint, 800);
                        let req = serde_json::json!({
                            "provider": embedding_provider,
                            "model": embedding_model,
                            "input_type": if embedding_provider == "cohere" {
                                JsonValue::String("search_document".to_string())
                            } else {
                                JsonValue::Null
                            },
                            "input_preview": preview,
                            "input_len": len,
                            "input_truncated": truncated,
                        });
                        let err_json = serde_json::from_str::<JsonValue>(&e)
                            .unwrap_or_else(|_| JsonValue::String(e.clone()));
                        let resp = serde_json::json!({ "error": err_json });
                        push_call(
                            &mut calls,
                            "hint",
                            Some(candidate_id.as_str()),
                            false,
                            req,
                            resp,
                        );

                        log::debug!(
                            "Intent router: failed to embed hint ({}): {}",
                            candidate_id,
                            e
                        );
                        continue;
                    }
                }
            };

            let sim = embeddings::cosine_similarity(&transcript_embedding, &hint_embedding);
            if let Some(sim) = sim {
                candidate_best = Some(candidate_best.map(|b| b.max(sim)).unwrap_or(sim));
            }
        }

        let Some(score) = candidate_best else {
            continue;
        };

        scores.push((candidate_id.clone(), score));

        match best {
            None => best = Some((candidate_id.clone(), score)),
            Some((_, best_score)) if score > best_score => {
                second_best = best.clone();
                best = Some((candidate_id.clone(), score));
            }
            _ => {
                if second_best
                    .as_ref()
                    .map(|(_, s)| score > *s)
                    .unwrap_or(true)
                {
                    second_best = Some((candidate_id.clone(), score));
                }
            }
        }
    }

    let Some((best_id, best_score)) = best else {
        let (router_req, router_resp) = build_router_payloads(&router_ctx, &None, &scores, &calls);
        return Some(RoutingDecision::embeddings(
            None,
            RoutingDecisionOutcome::NoDecision,
            scores,
            threshold,
            margin,
            router_req,
            router_resp,
        ));
    };

    if !pick_highest_score {
        if best_score < threshold {
            log::debug!(
                "Intent router: no preset met threshold (best {:.3} < {:.3})",
                best_score,
                threshold
            );
            let (router_req, router_resp) =
                build_router_payloads(&router_ctx, &None, &scores, &calls);
            return Some(RoutingDecision::embeddings(
                None,
                RoutingDecisionOutcome::NoDecision,
                scores,
                threshold,
                margin,
                router_req,
                router_resp,
            ));
        }

        if let Some((_, second_score)) = second_best {
            if best_score - second_score < margin {
                log::debug!(
                    "Intent router: ambiguous match (best {:.3}, second {:.3}, margin {:.3})",
                    best_score,
                    second_score,
                    margin
                );
                let (router_req, router_resp) =
                    build_router_payloads(&router_ctx, &None, &scores, &calls);
                return Some(RoutingDecision::embeddings(
                    None,
                    RoutingDecisionOutcome::Ambiguous,
                    scores,
                    threshold,
                    margin,
                    router_req,
                    router_resp,
                ));
            }
        }
    }

    let (selected, outcome) = if best_id == DEFAULT_CANDIDATE_ID {
        (None, RoutingDecisionOutcome::DefaultTarget)
    } else {
        (Some(best_id), RoutingDecisionOutcome::SelectedPreset)
    };

    let (router_req, router_resp) = build_router_payloads(&router_ctx, &selected, &scores, &calls);
    Some(RoutingDecision::embeddings(
        selected,
        outcome,
        scores,
        threshold,
        margin,
        router_req,
        router_resp,
    ))
}

pub(super) async fn route_preset_id_with_llm(
    profile: &crate::llm::ProgramPromptProfile,
    transcript: &str,
    provider: &dyn LlmProvider,
) -> Option<RoutingDecision> {
    let router = profile.router.as_ref()?;
    if !router.enabled || router.strategy != IntentRouterStrategy::Llm {
        return None;
    }

    let default_desc = profile
        .default_preset_description
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let candidate_count = profile.presets.len() + if default_desc.is_some() { 1 } else { 0 };
    if candidate_count < 2 {
        return None;
    }

    let transcript = transcript.trim();
    if transcript.is_empty() {
        return None;
    }

    // Keep this extremely constrained: output must select exactly one preset id (or "default").
    let mut options = String::new();
    if let Some(desc) = default_desc {
        options.push_str(&format!("- default: {}\n", desc));
    }
    for p in &profile.presets {
        let name = p.name.trim();
        let label = if name.is_empty() { p.id.as_str() } else { name };
        let hints = if p.routing_hints.is_empty() {
            String::new()
        } else {
            format!(" Hints: {}", p.routing_hints.join(" | "))
        };
        options.push_str(&format!("- {}: {}{}\n", p.id, label, hints));
    }

    let default_system = "You are an intent router. Choose the best preset id for the transcript.\n\nRules:\n- Output a JSON object matching the provided JSON Schema.\n- Choose exactly one preset_id from the allowed list.\n- If you are not confident, choose preset_id = 'default'.\n";

    let system = router
        .llm_system_prompt
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(default_system);

    let user = format!("Presets:\n{}\nTranscript:\n{}", options, transcript);

    // JSON schema (when supported) makes routing far more deterministic.
    // Enforce the allowed values at the schema level.
    let mut allowed: Vec<String> = Vec::new();
    // Prefer "default", but tolerate legacy "none".
    allowed.push("default".to_string());
    allowed.push("none".to_string());
    for p in &profile.presets {
        allowed.push(p.id.clone());
    }
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "preset_id": {
                "type": "string",
                "enum": allowed,
                "description": "Chosen preset id. Use 'default' to indicate no preset / default. (Legacy: 'none')"
            }
        },
        "required": ["preset_id"],
        "additionalProperties": false
    });

    let (system_preview, system_truncated, system_len) = preview_for_log(system, 1200);
    let (user_preview, user_truncated, user_len) = preview_for_log(&user, 2400);
    let request_json = serde_json::json!({
        "type": "llm",
        "provider": provider.name(),
        "model": provider.model(),
        "structured": true,
        "schema_name": "intent_router_choice",
        "messages": [
            {
                "role": "system",
                "content_preview": system_preview,
                "content_len": system_len,
                "content_truncated": system_truncated,
            },
            {
                "role": "user",
                "content_preview": user_preview,
                "content_len": user_len,
                "content_truncated": user_truncated,
            }
        ]
    });

    let v = match provider
        .complete_json_schema(
            system,
            &user,
            "intent_router_choice",
            "Select one preset id (or default) for the transcript.",
            schema,
        )
        .await
    {
        Ok(v) => v,
        Err(e) => {
            log::warn!("Intent router: LLM routing failed: {}", e);
            let response_json = serde_json::json!({
                "type": "llm",
                "error": e.to_string(),
            });
            return Some(RoutingDecision::llm(
                None,
                RoutingDecisionOutcome::Failed,
                request_json,
                response_json,
            ));
        }
    };

    let response_json = serde_json::json!({
        "type": "llm",
        "json": v,
    });

    let out = v
        .get("preset_id")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if out.eq_ignore_ascii_case("default") || out.eq_ignore_ascii_case("none") || out.is_empty() {
        return Some(RoutingDecision::llm(
            None,
            RoutingDecisionOutcome::DefaultTarget,
            request_json,
            response_json,
        ));
    }

    if profile.presets.iter().any(|p| p.id == out) {
        Some(RoutingDecision::llm(
            Some(out),
            RoutingDecisionOutcome::SelectedPreset,
            request_json,
            response_json,
        ))
    } else {
        log::debug!(
            "Intent router: LLM returned unknown preset id '{}'; ignored",
            out
        );
        Some(RoutingDecision::llm(
            None,
            RoutingDecisionOutcome::UnknownPreset,
            request_json,
            response_json,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::EmbeddingsError;
    use crate::llm::{LlmError, ProgramPreset, ProgramPromptProfile, PromptSections};
    use crate::settings::IntentRouterSettings;
    use async_trait::async_trait;
    use std::collections::HashMap;

    struct FakeEmbeddingsProvider {
        values: HashMap<String, Vec<f32>>,
        error_for: Option<String>,
    }

    impl FakeEmbeddingsProvider {
        fn new(values: &[(&str, Vec<f32>)]) -> Self {
            Self {
                values: values
                    .iter()
                    .map(|(key, value)| ((*key).to_string(), value.clone()))
                    .collect(),
                error_for: None,
            }
        }

        fn with_error_for(mut self, text: &str) -> Self {
            self.error_for = Some(text.to_string());
            self
        }
    }

    #[async_trait]
    impl EmbeddingsProvider for FakeEmbeddingsProvider {
        async fn embed_text(
            &self,
            text: &str,
            _input_type: Option<&str>,
        ) -> Result<(Vec<f32>, JsonValue, JsonValue), EmbeddingsError> {
            if self.error_for.as_deref() == Some(text) {
                return Err(EmbeddingsError::Api("planned failure".to_string()));
            }
            let embedding = self
                .values
                .get(text)
                .cloned()
                .unwrap_or_else(|| vec![0.0, 0.0]);
            Ok((
                embedding.clone(),
                serde_json::json!({ "text": text }),
                serde_json::json!({ "embedding_len": embedding.len() }),
            ))
        }

        fn name(&self) -> &'static str {
            "openai"
        }

        fn model(&self) -> &str {
            "fake-embedding-model"
        }
    }

    struct FakeLlmProvider {
        value: Result<JsonValue, String>,
    }

    #[async_trait]
    impl LlmProvider for FakeLlmProvider {
        async fn complete(
            &self,
            _system_prompt: &str,
            _user_message: &str,
        ) -> Result<String, LlmError> {
            Ok("{}".to_string())
        }

        async fn complete_json_schema(
            &self,
            _system_prompt: &str,
            _user_message: &str,
            _schema_name: &str,
            _schema_description: &str,
            _schema: JsonValue,
        ) -> Result<JsonValue, LlmError> {
            self.value
                .clone()
                .map_err(|error| LlmError::Api(error.to_string()))
        }

        fn name(&self) -> &'static str {
            "fake"
        }

        fn model(&self) -> &str {
            "fake-llm-model"
        }
    }

    fn preset(id: &str, hint: &str) -> ProgramPreset {
        ProgramPreset {
            id: id.to_string(),
            name: id.to_string(),
            routing_hints: vec![hint.to_string()],
            prompts: PromptSections::default(),
            rewrite_llm_enabled: true,
            stt_provider: None,
            stt_model: None,
            stt_language: None,
            stt_timeout_seconds: None,
            llm_provider: None,
            llm_model: None,
            openai_reasoning_effort: None,
            gemini_thinking_budget: None,
            gemini_thinking_level: None,
            anthropic_thinking_budget: None,
        }
    }

    fn profile(strategy: IntentRouterStrategy) -> ProgramPromptProfile {
        ProgramPromptProfile {
            id: "profile".to_string(),
            name: "Profile".to_string(),
            program_paths: vec![],
            prompts: PromptSections::default(),
            presets: vec![
                preset("email", "email hint"),
                preset("calendar", "calendar hint"),
            ],
            default_preset_id: None,
            default_preset_description: Some("default target".to_string()),
            default_target_rewrite_llm_enabled: true,
            active_preset_id: None,
            router: Some(IntentRouterSettings {
                enabled: true,
                strategy,
                embedding_provider: Some("openai".to_string()),
                embedding_model: Some("fake-embedding-model".to_string()),
                pick_highest_score: false,
                similarity_threshold: Some(0.75),
                similarity_margin: Some(0.10),
                llm_provider: Some("fake".to_string()),
                llm_model: Some("fake-llm-model".to_string()),
                llm_system_prompt: None,
                openai_reasoning_effort: None,
                gemini_thinking_budget: None,
                gemini_thinking_level: None,
                anthropic_thinking_budget: None,
            }),
            rewrite_llm_enabled: Some(true),
            stt_provider: None,
            stt_model: None,
            stt_language: None,
            stt_timeout_seconds: None,
            llm_provider: None,
            llm_model: None,
            openai_reasoning_effort: None,
            gemini_thinking_budget: None,
            gemini_thinking_level: None,
            anthropic_thinking_budget: None,
            quick_ask_provider: None,
            quick_ask_model: None,
            quick_ask_system_prompt: None,
            context_grab_method: None,
            rewrite_include_clipboard_context: None,
            quick_replace_include_clipboard_context: None,
            quick_ask_include_clipboard_context: None,
            rewrite_active_window_ocr_mode: None,
            quick_replace_active_window_ocr_mode: None,
            quick_ask_active_window_ocr_mode: None,
            quick_replace_enabled: None,
            quick_replace_provider: None,
            quick_replace_model: None,
            quick_replace_system_prompt: None,
            quick_ask_openai_reasoning_effort: None,
            quick_ask_gemini_thinking_budget: None,
            quick_ask_gemini_thinking_level: None,
            quick_ask_anthropic_thinking_budget: None,
        }
    }

    async fn embeddings_decision(
        provider: FakeEmbeddingsProvider,
        profile: &ProgramPromptProfile,
        transcript: &str,
    ) -> RoutingDecision {
        route_preset_id_with_embeddings(
            profile,
            transcript,
            &ProxySettings::default(),
            &HashMap::new(),
            &Arc::new(Mutex::new(HashMap::new())),
            None,
            Some(Arc::new(provider)),
        )
        .await
        .expect("router should be attempted")
    }

    #[tokio::test]
    async fn embeddings_selected_preset_default_no_decision_and_ambiguity_are_distinct() {
        let profile = profile(IntentRouterStrategy::Embeddings);

        let selected = embeddings_decision(
            FakeEmbeddingsProvider::new(&[
                ("send email", vec![1.0, 0.0]),
                ("email hint", vec![0.95, 0.05]),
                ("calendar hint", vec![0.0, 1.0]),
                ("default target", vec![0.0, 1.0]),
            ]),
            &profile,
            "send email",
        )
        .await;
        assert_eq!(selected.outcome, RoutingDecisionOutcome::SelectedPreset);
        assert_eq!(selected.selected_preset_id.as_deref(), Some("email"));

        let default = embeddings_decision(
            FakeEmbeddingsProvider::new(&[
                ("general words", vec![1.0, 0.0]),
                ("email hint", vec![0.0, 1.0]),
                ("calendar hint", vec![0.0, 1.0]),
                ("default target", vec![0.95, 0.05]),
            ]),
            &profile,
            "general words",
        )
        .await;
        assert_eq!(default.outcome, RoutingDecisionOutcome::DefaultTarget);
        assert_eq!(default.selected_preset_id, None);

        let no_decision = embeddings_decision(
            FakeEmbeddingsProvider::new(&[
                ("unclear", vec![1.0, 0.0]),
                ("email hint", vec![0.1, 0.9]),
                ("calendar hint", vec![0.0, 1.0]),
                ("default target", vec![0.0, 1.0]),
            ]),
            &profile,
            "unclear",
        )
        .await;
        assert_eq!(no_decision.outcome, RoutingDecisionOutcome::NoDecision);

        let ambiguous = embeddings_decision(
            FakeEmbeddingsProvider::new(&[
                ("message", vec![1.0, 0.0]),
                ("email hint", vec![0.95, 0.05]),
                ("calendar hint", vec![0.90, 0.10]),
                ("default target", vec![0.0, 1.0]),
            ]),
            &profile,
            "message",
        )
        .await;
        assert_eq!(ambiguous.outcome, RoutingDecisionOutcome::Ambiguous);
    }

    #[tokio::test]
    async fn embeddings_failure_records_failed_decision_diagnostics() {
        let profile = profile(IntentRouterStrategy::Embeddings);
        let decision = embeddings_decision(
            FakeEmbeddingsProvider::new(&[]).with_error_for("boom"),
            &profile,
            "boom",
        )
        .await;

        assert_eq!(decision.outcome, RoutingDecisionOutcome::Failed);
        assert_eq!(decision.selected_preset_id, None);
        assert_eq!(decision.response_json["outcome"], "failed");
    }

    #[tokio::test]
    async fn llm_selected_default_failed_and_unknown_outcomes_are_distinct() {
        let profile = profile(IntentRouterStrategy::Llm);

        let selected = route_preset_id_with_llm(
            &profile,
            "send email",
            &FakeLlmProvider {
                value: Ok(serde_json::json!({ "preset_id": "email" })),
            },
        )
        .await
        .expect("router should be attempted");
        assert_eq!(selected.outcome, RoutingDecisionOutcome::SelectedPreset);
        assert_eq!(selected.selected_preset_id.as_deref(), Some("email"));

        let default = route_preset_id_with_llm(
            &profile,
            "use default",
            &FakeLlmProvider {
                value: Ok(serde_json::json!({ "preset_id": "default" })),
            },
        )
        .await
        .expect("router should be attempted");
        assert_eq!(default.outcome, RoutingDecisionOutcome::DefaultTarget);
        assert_eq!(default.selected_preset_id, None);

        let failed = route_preset_id_with_llm(
            &profile,
            "fail",
            &FakeLlmProvider {
                value: Err("planned failure".to_string()),
            },
        )
        .await
        .expect("router should be attempted");
        assert_eq!(failed.outcome, RoutingDecisionOutcome::Failed);
        assert_eq!(failed.response_json["outcome"], "failed");

        let unknown = route_preset_id_with_llm(
            &profile,
            "unknown",
            &FakeLlmProvider {
                value: Ok(serde_json::json!({ "preset_id": "bogus" })),
            },
        )
        .await
        .expect("router should be attempted");
        assert_eq!(unknown.outcome, RoutingDecisionOutcome::UnknownPreset);
        assert_eq!(unknown.selected_preset_id, None);
    }
}
