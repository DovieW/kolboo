//! Persistent usage/cost stats ledger.
//!
//! This is intentionally separate from `request_log`:
//! - request logs are in-memory and meant for debugging.
//! - stats are persisted (for usage analytics / cost reporting).

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::cost::openai as openai_cost;
use crate::cost::groq as groq_cost;
use crate::cost::aquavoice as aquavoice_cost;
use crate::cost::gemini as gemini_cost;
use crate::cost::anthropic as anthropic_cost;
use crate::cost::deepgram as deepgram_cost;
use crate::cost::assemblyai as assemblyai_cost;
use crate::cost::speechmatics as speechmatics_cost;
use crate::cost::fireworks as fireworks_cost;
use tauri::AppHandle;
use tauri::{Manager, Emitter};
use crate::request_log::RequestLogStore;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CostKind {
    Stt,
    Llm,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    Success,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub input_audio_tokens: u64,
    pub output_audio_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEvent {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub request_id: String,
    pub kind: CostKind,
    pub provider: String,
    pub model: Option<String>,
    #[serde(default)]
    pub is_free_tier: bool,
    pub status: EventStatus,

    // Usage
    pub audio_duration_secs: Option<f64>,
    pub tokens: Option<TokenUsage>,

    // Estimated costs
    pub estimated_cost_usd_micros: Option<openai_cost::UsdMicros>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_cost_breakdown_openai: Option<openai_cost::OpenAiCostBreakdown>,
}

impl CostEvent {
    pub fn new(
        request_id: String,
        kind: CostKind,
        provider: String,
        model: Option<String>,
        status: EventStatus,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            request_id,
            kind,
            provider,
            model,
            is_free_tier: false,
            status,
            audio_duration_secs: None,
            tokens: None,
            estimated_cost_usd_micros: None,
            estimated_cost_breakdown_openai: None,
        }
    }
}

fn is_free_tier_call(app: &AppHandle, provider: &str) -> bool {
    #[cfg(desktop)]
    {
        // Default to true, matching UI expectations.
        return match provider {
            "cerebras" => crate::get_setting_from_store(app, "cerebras_free_tier", true),
            "groq" => crate::get_setting_from_store(app, "groq_free_tier", true),
            "elevenlabs" => crate::get_setting_from_store(app, "elevenlabs_free_tier", true),
            "cohere" => crate::get_setting_from_store(app, "cohere_free_tier", true),
            "assemblyai" => crate::get_setting_from_store(app, "assemblyai_free_tier", true),
            "speechmatics" => crate::get_setting_from_store(app, "speechmatics_free_tier", true),
            _ => false,
        };
    }

    #[cfg(not(desktop))]
    {
        false
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StatsRetentionConfig {
    pub time_retention: Option<ChronoDuration>,
    pub max_bytes: u64,
}

/// Persistent stats store.
///
/// Files are stored under `<app_data_dir>/stats/`.
/// We shard by day in JSONL for cheap appends and easy retention.
#[derive(Debug, Clone)]
pub struct StatsStore {
    dir: PathBuf,
}

impl StatsStore {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let dir = app_data_dir.join("stats");
        if let Err(e) = fs::create_dir_all(&dir) {
            log::warn!("Failed to create stats dir {:?}: {}", dir, e);
        }
        Self { dir }
    }

    pub fn append_cost_event(&self, event: &CostEvent) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;

        let date = event.created_at.format("%Y-%m-%d");
        let file_path = self.dir.join(format!("cost-events-{}.jsonl", date));
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .map_err(|e| format!("Failed to open stats file {:?}: {}", file_path, e))?;

        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, event).map_err(|e| e.to_string())?;
        writer.write_all(b"\n").map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn prune(&self, cfg: StatsRetentionConfig) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;

        let now = Utc::now();

        // 1) Time-based retention: delete old daily shard files.
        if let Some(retention) = cfg.time_retention {
            let cutoff = now - retention;
            let entries = fs::read_dir(&self.dir).map_err(|e| e.to_string())?;
            for entry in entries {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !name.starts_with("cost-events-") || !name.ends_with(".jsonl") {
                    continue;
                }

                // Parse YYYY-MM-DD from filename.
                let date_part = name
                    .trim_start_matches("cost-events-")
                    .trim_end_matches(".jsonl");
                if let Ok(date) = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
                    // Compare by date; if it's strictly older than the cutoff date, delete.
                    if date < cutoff.date_naive() {
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }

        // 2) Max-bytes retention: delete oldest shard files until under budget.
        let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
        for entry in fs::read_dir(&self.dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let meta = entry.metadata().map_err(|e| e.to_string())?;
            let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            files.push((path, meta.len(), modified));
        }

        files.sort_by_key(|(_, _, modified)| *modified);

        let mut total_bytes: u128 = files.iter().map(|(_, sz, _)| *sz as u128).sum();
        let max_bytes = cfg.max_bytes.max(1) as u128;
        for (path, sz, _) in files {
            if total_bytes <= max_bytes {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                total_bytes = total_bytes.saturating_sub(sz as u128);
            }
        }

        Ok(())
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[cfg(desktop)]
pub fn read_stats_retention_config(app: &tauri::AppHandle) -> StatsRetentionConfig {
    let unit: String = crate::get_setting_from_store(app, "stats_retention_unit", "days".into());
    let value: f64 = crate::get_setting_from_store(app, "stats_retention_value", 30.0f64);
    let max_bytes: u64 = crate::get_setting_from_store(app, "stats_retention_max_bytes", 50_000_000u64);

    let value = if value.is_finite() { value.max(0.0) } else { 0.0 };

    let time_retention = if value == 0.0 {
        None
    } else if unit == "hours" {
        Some(ChronoDuration::milliseconds((value * 3600.0 * 1000.0) as i64))
    } else {
        // Default: days
        Some(ChronoDuration::milliseconds((value * 24.0 * 3600.0 * 1000.0) as i64))
    };

    StatsRetentionConfig {
        time_retention,
        max_bytes,
    }
}

#[cfg(not(desktop))]
pub fn read_stats_retention_config(_app: &tauri::AppHandle) -> StatsRetentionConfig {
    // Mobile/non-desktop builds don't use the store plugin. Keep a sensible default.
    StatsRetentionConfig {
        time_retention: Some(ChronoDuration::days(30)),
        max_bytes: 50_000_000,
    }
}

/// Best-effort WAV duration computation.
///
/// Returns duration in seconds.
pub fn wav_duration_secs(wav_bytes: &[u8]) -> Option<f64> {
    use std::io::Cursor;

    let reader = hound::WavReader::new(Cursor::new(wav_bytes)).ok()?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as u64;
    let sample_rate = spec.sample_rate.max(1) as u64;
    let samples = reader.duration() as u64;

    let frames = samples / channels;
    Some(frames as f64 / sample_rate as f64)
}

/// Parse OpenAI usage information out of a response JSON.
///
/// Supports both:
/// - Responses API: usage.input_tokens/output_tokens + *_details
/// - Chat Completions API: usage.prompt_tokens/completion_tokens
pub fn parse_openai_usage_from_response_json(v: &JsonValue) -> Option<openai_cost::OpenAiUsage> {
    let usage = v.get("usage")?;

    // Chat Completions shape
    if usage.get("prompt_tokens").is_some() || usage.get("completion_tokens").is_some() {
        let prompt = usage.get("prompt_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
        let completion = usage
            .get("completion_tokens")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        return Some(openai_cost::OpenAiUsage {
            input_tokens: prompt,
            output_tokens: completion,
            cached_input_tokens: 0,
            input_audio_tokens: 0,
            output_audio_tokens: 0,
        });
    }

    // Responses shape
    let input_tokens = usage.get("input_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let cached_input_tokens = usage
        .get("input_tokens_details")
        .and_then(|d| d.get("cached_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let input_audio_tokens = usage
        .get("input_tokens_details")
        .and_then(|d| d.get("audio_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let output_audio_tokens = usage
        .get("output_tokens_details")
        .and_then(|d| d.get("audio_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    Some(openai_cost::OpenAiUsage {
        input_tokens,
        output_tokens,
        cached_input_tokens,
        input_audio_tokens,
        output_audio_tokens,
    })
}

/// Parse Gemini token usage information out of a Gemini `models.generateContent` response JSON.
///
/// Gemini responses include a top-level `usageMetadata` object that looks like:
/// - `promptTokenCount`
/// - `candidatesTokenCount`
/// - `totalTokenCount`
///
/// We map those into an OpenAI-style usage struct for downstream cost estimators.
pub fn parse_gemini_usage_from_response_json(v: &JsonValue) -> Option<openai_cost::OpenAiUsage> {
    let usage = v
        .get("usageMetadata")
        .or_else(|| v.get("usage_metadata"))?;

    let prompt = usage
        .get("promptTokenCount")
        .or_else(|| usage.get("prompt_token_count"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let candidates = usage
        .get("candidatesTokenCount")
        .or_else(|| usage.get("candidates_token_count"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    Some(openai_cost::OpenAiUsage {
        input_tokens: prompt,
        output_tokens: candidates,
        cached_input_tokens: 0,
        input_audio_tokens: 0,
        output_audio_tokens: 0,
    })
}

/// Parse Anthropic Claude Messages API token usage out of a response JSON.
///
/// Anthropic responses include a top-level `usage` object with fields like:
/// - `input_tokens`
/// - `output_tokens`
/// - `cache_creation_input_tokens`
/// - `cache_read_input_tokens`
///
/// When prompt caching is used, responses may also include:
///
/// ```json
/// "usage": {
///   "cache_creation": {
///     "ephemeral_1h_input_tokens": 0,
///     "ephemeral_5m_input_tokens": 0
///   }
/// }
/// ```
pub fn parse_anthropic_usage_from_response_json(v: &JsonValue) -> Option<anthropic_cost::AnthropicUsage> {
    let usage = v.get("usage")?;

    let input_tokens = usage.get("input_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
    let output_tokens = usage.get("output_tokens").and_then(|x| x.as_u64()).unwrap_or(0);

    let cache_read_input_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let cache_creation_total = usage
        .get("cache_creation_input_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let mut cache_creation_5m_input_tokens = usage
        .get("cache_creation")
        .and_then(|c| c.get("ephemeral_5m_input_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let cache_creation_1h_input_tokens = usage
        .get("cache_creation")
        .and_then(|c| c.get("ephemeral_1h_input_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    // If the split isn't present, fall back to the aggregated count.
    if cache_creation_5m_input_tokens == 0 && cache_creation_1h_input_tokens == 0 {
        cache_creation_5m_input_tokens = cache_creation_total;
    } else {
        // If the totals don't match (API evolution), assign any remainder to 5m.
        let split_sum = cache_creation_5m_input_tokens.saturating_add(cache_creation_1h_input_tokens);
        if cache_creation_total > split_sum {
            cache_creation_5m_input_tokens = cache_creation_5m_input_tokens
                .saturating_add(cache_creation_total.saturating_sub(split_sum));
        }
    }

    Some(anthropic_cost::AnthropicUsage {
        input_tokens,
        output_tokens,
        cache_creation_5m_input_tokens,
        cache_creation_1h_input_tokens,
        cache_read_input_tokens,
    })
}

/// Parse OpenAI STT duration (seconds) from transcription responses.
///
/// OpenAI transcription endpoints may return:
///
/// ```json
/// { "usage": { "seconds": 2, "type": "duration" } }
/// ```
pub fn parse_openai_stt_duration_secs_from_response_json(v: &JsonValue) -> Option<f64> {
    let usage = v.get("usage")?;
    let ty = usage.get("type").and_then(|x| x.as_str()).unwrap_or("");
    if ty != "duration" {
        return None;
    }

    usage
        .get("seconds")
        .and_then(|x| x.as_f64().or_else(|| x.as_u64().map(|u| u as f64)))
        .filter(|s| s.is_finite() && *s >= 0.0)
}

/// Parse Deepgram STT duration (seconds) from a `/v1/listen` response.
///
/// Deepgram includes `metadata.duration` in seconds:
///
/// ```json
/// { "metadata": { "duration": 25.933313, ... } }
/// ```
pub fn parse_deepgram_stt_duration_secs_from_response_json(v: &JsonValue) -> Option<f64> {
    v.get("metadata")?
        .get("duration")
        .and_then(|x| x.as_f64().or_else(|| x.as_u64().map(|u| u as f64)))
        .filter(|s| s.is_finite() && *s >= 0.0)
}

/// Centralized helper: emit cost events for the *current* request log.
///
/// This lives in `stats` so any command/path (pipeline flows, test buttons, analyze UI)
/// can call it without duplicating logic.
pub fn emit_cost_events_for_current_request(
    app: &AppHandle,
    status: EventStatus,
    wav_bytes: Option<&[u8]>,
) {
    log::info!("emit_cost_events_for_current_request called with status {:?}", status);

    let Some(stats_store) = app.try_state::<StatsStore>() else {
        log::warn!("StatsStore not available");
        return;
    };

    let Some(log_store) = app.try_state::<RequestLogStore>() else {
        log::warn!("RequestLogStore not available");
        return;
    };

    let inputs = log_store.with_current(|log| {
        let (llm_provider, llm_model, llm_response_json) = match log.kind {
            crate::request_log::RequestKind::QuickAsk => (
                log.quick_ask_provider.clone().or_else(|| log.llm_provider.clone()),
                log.quick_ask_model.clone().or_else(|| log.llm_model.clone()),
                log.quick_ask_response_json
                    .clone()
                    .or_else(|| log.llm_response_json.clone()),
            ),
            crate::request_log::RequestKind::QuickReplace => (
                log.quick_replace_provider.clone().or_else(|| log.llm_provider.clone()),
                log.quick_replace_model.clone().or_else(|| log.llm_model.clone()),
                log.quick_replace_response_json
                    .clone()
                    .or_else(|| log.llm_response_json.clone()),
            ),
            _ => (
                log.llm_provider.clone(),
                log.llm_model.clone(),
                log.llm_response_json.clone(),
            ),
        };

        CurrentInputsForStats {
            request_id: log.id.clone(),
            stt_provider: log.stt_provider.clone(),
            stt_model: log.stt_model.clone(),
            stt_response_json: log.stt_response_json.clone(),
            llm_provider,
            llm_model,
            llm_response_json,
        }
    });

    // If there's no active request, nothing to do.
    let Some(inputs) = inputs else {
        log::warn!("No current request log available");
        return;
    };

    log::info!("Processing cost events for request {}", inputs.request_id);

    // Prefer WAV-derived duration (ground truth), but fall back to provider-reported duration
    // (e.g. OpenAI transcription `usage.seconds`) when WAV bytes are unavailable.
    let mut audio_secs = wav_bytes.and_then(wav_duration_secs);

    // If we successfully append any cost event, notify the UI so it can invalidate cached stats.
    let mut any_stats_written = false;

    // STT cost event
    {
        let mut ev = CostEvent::new(
            inputs.request_id.clone(),
            CostKind::Stt,
            inputs.stt_provider.clone(),
            inputs.stt_model.clone(),
            status,
        );

        ev.is_free_tier = is_free_tier_call(app, ev.provider.as_str());

        // Provider-specific duration fallback.
        if audio_secs.is_none() && inputs.stt_provider == "openai" {
            if let Some(resp) = inputs.stt_response_json.as_ref() {
                audio_secs = parse_openai_stt_duration_secs_from_response_json(resp);
            }
        }

        if audio_secs.is_none() && inputs.stt_provider == "deepgram" {
            if let Some(resp) = inputs.stt_response_json.as_ref() {
                audio_secs = parse_deepgram_stt_duration_secs_from_response_json(resp);
            }
        }

        ev.audio_duration_secs = audio_secs;

        if inputs.stt_provider == "openai" {
            if let (Some(model), Some(resp)) = (ev.model.as_deref(), inputs.stt_response_json.as_ref()) {
                if let Some(u) = parse_openai_usage_from_response_json(resp) {
                    ev.tokens = Some(TokenUsage {
                        input_tokens: u.input_tokens,
                        output_tokens: u.output_tokens,
                        cached_input_tokens: u.cached_input_tokens,
                        input_audio_tokens: u.input_audio_tokens,
                        output_audio_tokens: u.output_audio_tokens,
                    });

                    if let Some(breakdown) = openai_cost::estimate_cost_from_usage(model, u) {
                        ev.estimated_cost_usd_micros = Some(breakdown.total_usd_micros);
                        ev.estimated_cost_breakdown_openai = Some(breakdown);
                    }
                }
            }

            // If no token-usage-based estimate exists (e.g. Whisper transcription endpoint),
            // fall back to transcription-per-minute pricing if available.
            if ev.estimated_cost_usd_micros.is_none() {
                if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
                    if let Some(micros) = openai_cost::estimate_transcription_cost_from_audio_secs(model, secs) {
                        ev.estimated_cost_usd_micros = Some(micros);
                    }
                }
            }
        }

        if inputs.stt_provider == "groq" {
            // Even when marked free-tier, still estimate the list-price cost so users can
            // optionally include free-tier calls in Stats.
            if ev.estimated_cost_usd_micros.is_none() {
                if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
                    if let Some(micros) = groq_cost::estimate_stt_cost_from_audio_secs(model, secs) {
                        ev.estimated_cost_usd_micros = Some(micros);
                    }
                }
            }
        }

        if inputs.stt_provider == "deepgram" {
            if ev.estimated_cost_usd_micros.is_none() {
                if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
                    if let Some(micros) = deepgram_cost::estimate_stt_cost_from_audio_secs(model, secs) {
                        ev.estimated_cost_usd_micros = Some(micros);
                    }
                }
            }
        }

        if inputs.stt_provider == "aquavoice" {
            if ev.estimated_cost_usd_micros.is_none() {
                if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
                    if let Some(micros) = aquavoice_cost::estimate_stt_cost_from_audio_secs(model, secs) {
                        ev.estimated_cost_usd_micros = Some(micros);
                    }
                }
            }
        }

        if inputs.stt_provider == "assemblyai" {
            if ev.estimated_cost_usd_micros.is_none() {
                if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
                    if let Some(micros) = assemblyai_cost::estimate_stt_cost_from_audio_secs(model, secs) {
                        ev.estimated_cost_usd_micros = Some(micros);
                    }
                }
            }
        }

        if inputs.stt_provider == "speechmatics" {
            // Even when marked free-tier, still estimate list-price cost so users can
            // optionally include free-tier calls in Stats.
            if ev.estimated_cost_usd_micros.is_none() {
                if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
                    if let Some(micros) = speechmatics_cost::estimate_stt_cost_from_audio_secs(model, secs) {
                        ev.estimated_cost_usd_micros = Some(micros);
                    }
                }
            }
        }

        if inputs.stt_provider == "fireworks" {
            if ev.estimated_cost_usd_micros.is_none() {
                if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
                    if let Some(micros) = fireworks_cost::estimate_stt_cost_from_audio_secs(model, secs) {
                        ev.estimated_cost_usd_micros = Some(micros);
                    }
                }
            }
        }

        // Surface per-call pricing info in the in-memory request log so the UI can show it.
        let _ = log_store.with_current(|log| {
            log.stt_is_free_tier = ev.is_free_tier;
            log.stt_estimated_cost_usd_micros = ev.estimated_cost_usd_micros;
        });

        if let Err(e) = stats_store.append_cost_event(&ev) {
            log::warn!("Failed to append STT cost event: {}", e);
        } else {
            any_stats_written = true;
            log::info!("Successfully wrote STT cost event for request {}, cost: {:?} micros", inputs.request_id, ev.estimated_cost_usd_micros);
        }
    }

    // LLM cost event (only if an LLM provider/model is set)
    if let (Some(llm_provider), Some(llm_model)) = (inputs.llm_provider.as_deref(), inputs.llm_model.as_deref()) {
        let mut ev = CostEvent::new(
            inputs.request_id.clone(),
            CostKind::Llm,
            llm_provider.to_string(),
            Some(llm_model.to_string()),
            status,
        );

        ev.is_free_tier = is_free_tier_call(app, ev.provider.as_str());

        if llm_provider == "openai" {
            if let (Some(model), Some(resp)) = (ev.model.as_deref(), inputs.llm_response_json.as_ref()) {
                if let Some(u) = parse_openai_usage_from_response_json(resp) {
                    ev.tokens = Some(TokenUsage {
                        input_tokens: u.input_tokens,
                        output_tokens: u.output_tokens,
                        cached_input_tokens: u.cached_input_tokens,
                        input_audio_tokens: u.input_audio_tokens,
                        output_audio_tokens: u.output_audio_tokens,
                    });
                    if let Some(breakdown) = openai_cost::estimate_cost_from_usage(model, u) {
                        ev.estimated_cost_usd_micros = Some(breakdown.total_usd_micros);
                        ev.estimated_cost_breakdown_openai = Some(breakdown);
                    }
                }
            }
        }

        if llm_provider == "groq" {
            // Even when marked free-tier, still estimate the list-price cost so users can
            // optionally include free-tier calls in Stats.
            if let (Some(model), Some(resp)) = (ev.model.as_deref(), inputs.llm_response_json.as_ref()) {
                if let Some(u) = parse_openai_usage_from_response_json(resp) {
                    ev.tokens = Some(TokenUsage {
                        input_tokens: u.input_tokens,
                        output_tokens: u.output_tokens,
                        cached_input_tokens: u.cached_input_tokens,
                        input_audio_tokens: u.input_audio_tokens,
                        output_audio_tokens: u.output_audio_tokens,
                    });

                    if let Some(micros) = groq_cost::estimate_llm_cost_from_usage(model, u) {
                        ev.estimated_cost_usd_micros = Some(micros);
                    }
                }
            }
        }

        if llm_provider == "gemini" {
            if let (Some(model), Some(resp)) = (ev.model.as_deref(), inputs.llm_response_json.as_ref()) {
                if let Some(u) = parse_gemini_usage_from_response_json(resp) {
                    ev.tokens = Some(TokenUsage {
                        input_tokens: u.input_tokens,
                        output_tokens: u.output_tokens,
                        cached_input_tokens: 0,
                        input_audio_tokens: 0,
                        output_audio_tokens: 0,
                    });

                    if let Some(micros) = gemini_cost::estimate_llm_cost_from_usage(model, u) {
                        ev.estimated_cost_usd_micros = Some(micros);
                    }
                }
            }
        }

        if llm_provider == "anthropic" {
            if let (Some(model), Some(resp)) = (ev.model.as_deref(), inputs.llm_response_json.as_ref()) {
                if let Some(u) = parse_anthropic_usage_from_response_json(resp) {
                    let total_input = u.total_input_tokens_for_tier();
                    ev.tokens = Some(TokenUsage {
                        input_tokens: total_input,
                        output_tokens: u.output_tokens,
                        cached_input_tokens: u.cache_read_input_tokens,
                        input_audio_tokens: 0,
                        output_audio_tokens: 0,
                    });

                    if let Some(micros) = anthropic_cost::estimate_llm_cost_from_usage(model, u) {
                        ev.estimated_cost_usd_micros = Some(micros);
                    }
                }
            }
        }

        if llm_provider == "fireworks" {
            if let (Some(model), Some(resp)) = (ev.model.as_deref(), inputs.llm_response_json.as_ref()) {
                if let Some(u) = parse_openai_usage_from_response_json(resp) {
                    ev.tokens = Some(TokenUsage {
                        input_tokens: u.input_tokens,
                        output_tokens: u.output_tokens,
                        cached_input_tokens: u.cached_input_tokens,
                        input_audio_tokens: u.input_audio_tokens,
                        output_audio_tokens: u.output_audio_tokens,
                    });

                    if let Some(micros) = fireworks_cost::estimate_llm_cost_from_usage(model, u) {
                        ev.estimated_cost_usd_micros = Some(micros);
                    }
                }
            }
        }

        if let Err(e) = stats_store.append_cost_event(&ev) {
            log::warn!("Failed to append LLM cost event: {}", e);
        } else {
            any_stats_written = true;
        }

        // Surface per-call pricing info in the in-memory request log so the UI can show it.
        let _ = log_store.with_current(|log| {
            log.llm_is_free_tier = ev.is_free_tier;
            log.llm_estimated_cost_usd_micros = ev.estimated_cost_usd_micros;
        });
    }

    // Best-effort pruning after each write.
    let cfg = read_stats_retention_config(app);
    let _ = stats_store.prune(cfg);

    if any_stats_written {
        let _ = app.emit("stats-changed", ());
    }
}

// Small helper type for the closure above to avoid copying the whole RequestLog definition.
#[derive(Debug, Clone)]
struct CurrentInputsForStats {
    request_id: String,
    stt_provider: String,
    stt_model: Option<String>,
    stt_response_json: Option<JsonValue>,
    llm_provider: Option<String>,
    llm_model: Option<String>,
    llm_response_json: Option<JsonValue>,
}
