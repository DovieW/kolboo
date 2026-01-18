//! Request logging system for debugging and troubleshooting.
//!
//! Captures detailed logs for each transcription request including:
//! - Request metadata (timestamp, provider, model)
//! - Audio information (duration, sample rate, size)
//! - API request/response details
//! - Timing information
//! - Errors if any

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::VecDeque;
use std::error::Error;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

fn should_redact_key(key: &str) -> bool {
    let k = key.trim().to_lowercase();
    k == "authorization"
        || k == "proxy-authorization"
        || k == "x-api-key"
        || k == "api_key"
        || k == "api-key"
        || k.ends_with("_api_key")
        || k == "access_token"
        || k == "refresh_token"
        || k == "id_token"
}

fn redact_string_value(s: &str) -> Option<&'static str> {
    let trimmed = s.trim();

    // Common token formats
    if trimmed.starts_with("Bearer ") {
        return Some("<redacted>");
    }
    // OpenAI-ish prefixes, etc.
    if trimmed.starts_with("sk-") || trimmed.starts_with("rk-") {
        return Some("<redacted>");
    }

    None
}

/// Best-effort redaction of secrets from JSON payloads stored in request logs.
///
/// This is intentionally conservative and errs on the side of removing sensitive fields.
pub fn redact_json(value: JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(mut map) => {
            // Redact by key name.
            for (k, v) in map.iter_mut() {
                if should_redact_key(k) {
                    *v = JsonValue::String("<redacted>".to_string());
                    continue;
                }

                // Recurse.
                let next = std::mem::take(v);
                *v = redact_json(next);
            }
            JsonValue::Object(map)
        }
        JsonValue::Array(arr) => JsonValue::Array(arr.into_iter().map(redact_json).collect()),
        JsonValue::String(s) => {
            if let Some(replacement) = redact_string_value(&s) {
                JsonValue::String(replacement.to_string())
            } else {
                JsonValue::String(s)
            }
        }
        other => other,
    }
}

fn redact_request_log_json_fields(mut log: RequestLog) -> RequestLog {
    log.stt_request_json = log.stt_request_json.map(redact_json);
    log.stt_response_json = log.stt_response_json.map(redact_json);
    log.llm_request_json = log.llm_request_json.map(redact_json);
    log.llm_response_json = log.llm_response_json.map(redact_json);
    log.router_request_json = log.router_request_json.map(redact_json);
    log.router_response_json = log.router_response_json.map(redact_json);
    log.quick_ask_request_json = log.quick_ask_request_json.map(redact_json);
    log.quick_ask_response_json = log.quick_ask_response_json.map(redact_json);
    log.quick_replace_request_json = log.quick_replace_request_json.map(redact_json);
    log.quick_replace_response_json = log.quick_replace_response_json.map(redact_json);
    log
}

/// Default number of request logs to keep (matches UI default)
const DEFAULT_MAX_LOGS: usize = 50;

/// Defensive hard cap for request logs kept in memory.
///
/// Even when using time-based retention, we don't want unbounded growth. Also,
/// request logs can include large JSON payloads, so keep this conservative.
const HARD_MAX_LOGS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestLogsRetentionMode {
    Amount,
    Time,
}

#[derive(Debug, Clone, Copy)]
pub struct RequestLogsRetentionConfig {
    pub mode: RequestLogsRetentionMode,
    /// Only used when mode == Amount.
    pub amount: usize,
    /// Only used when mode == Time.
    /// None means keep forever (time-based retention disabled).
    pub time_retention: Option<ChronoDuration>,
}

impl Default for RequestLogsRetentionConfig {
    fn default() -> Self {
        Self {
            mode: RequestLogsRetentionMode::Amount,
            amount: DEFAULT_MAX_LOGS,
            // Not used by default mode, but keep a sane value.
            time_retention: Some(ChronoDuration::days(7)),
        }
    }
}

/// A single log entry within a request
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub message: String,
    pub details: Option<String>,
}

/// Log level for entries
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Router score for a preset candidate.
///
/// - `score` is strategy-dependent.
///   - embeddings router: cosine similarity (0..=1-ish)
///   - llm router: currently `None` (no numeric scoring)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RouterPresetScore {
    pub preset_id: String,
    pub preset_name: String,
    #[serde(default)]
    pub score: Option<f32>,
    #[serde(default)]
    pub selected: bool,
}

/// High-level kind of request represented by a `RequestLog`.
///
/// Most logs represent the main pipeline transcription+rewrite flow.
/// Quick Ask sessions are also backed by the pipeline, but include an additional
/// answer-generation step and should be surfaced separately in the UI.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RequestKind {
    #[default]
    Transcription,
    QuickAsk,
    QuickReplace,
}

/// A complete request log containing all entries for a single transcription request
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequestLog {
    /// Unique ID for this request
    pub id: String,

    /// High-level request kind for UI grouping/filtering.
    #[serde(default)]
    pub kind: RequestKind,
    /// When the request started
    pub started_at: DateTime<Utc>,
    /// When request *processing* started (excludes recording time).
    ///
    /// For the main pipeline flow, the request log is created at recording-start,
    /// but the user-facing "Total" duration should represent only the backend
    /// processing time (stop -> STT/LLM -> done).
    #[serde(default)]
    pub processing_started_at: Option<DateTime<Utc>>,
    /// When the request completed (if finished)
    #[serde(rename = "ended_at")]
    pub completed_at: Option<DateTime<Utc>>,
    /// STT provider used
    pub stt_provider: String,
    /// STT model used
    pub stt_model: Option<String>,
    /// LLM provider used (if formatting enabled)
    pub llm_provider: Option<String>,
    /// LLM model used
    pub llm_model: Option<String>,

    /// Prompt profile id used for this request.
    ///
    /// "default" means no per-program profile matched.
    #[serde(default)]
    pub profile_id: Option<String>,
    /// Prompt profile display name used for this request.
    #[serde(default)]
    pub profile_name: Option<String>,

    /// Preset id selected for this request (if any).
    ///
    /// When None, the request used the profile/global defaults ("Default" in UI).
    #[serde(default)]
    pub preset_id: Option<String>,
    /// Preset display name selected for this request (if any).
    #[serde(default)]
    pub preset_name: Option<String>,
    /// Audio duration in seconds
    pub audio_duration_secs: Option<f32>,
    /// Audio file size in bytes
    pub audio_size_bytes: Option<usize>,
    /// Sample rate of the audio
    pub sample_rate: Option<u32>,
    /// Raw transcript from STT
    pub raw_transcript: Option<String>,
    /// Formatted transcript from LLM (if used)
    #[serde(rename = "final_text")]
    pub formatted_transcript: Option<String>,

    /// LLM rewrite: clipboard text that was included as context for the rewrite (when enabled).
    #[serde(default)]
    pub rewrite_clipboard_context: Option<String>,

    /// Quick Ask: the question sent to the answering LLM (usually based on the transcript).
    #[serde(default)]
    pub quick_ask_question: Option<String>,
    /// Quick Ask: highlighted text context that was attached to the question and sent to the answering LLM.
    ///
    /// This is intentionally bounded (see Quick Ask prompt assembly) to keep request logs usable.
    #[serde(default)]
    pub quick_ask_context_text: Option<String>,
    /// Quick Ask: clipboard text that was included as additional context (when enabled).
    #[serde(default)]
    pub quick_ask_clipboard_context: Option<String>,
    /// Quick Ask: the answer returned by the answering LLM.
    #[serde(default)]
    pub quick_ask_answer: Option<String>,
    /// Quick Ask: provider used for the answering LLM.
    #[serde(default)]
    pub quick_ask_provider: Option<String>,
    /// Quick Ask: model used for the answering LLM.
    #[serde(default)]
    pub quick_ask_model: Option<String>,
    /// Quick Ask: duration of the answering LLM call in milliseconds.
    #[serde(default)]
    pub quick_ask_duration_ms: Option<u64>,

    /// Quick Ask: payload sent to the answering LLM.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quick_ask_request_json: Option<JsonValue>,
    /// Quick Ask: payload received from the answering LLM.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quick_ask_response_json: Option<JsonValue>,

    /// Quick Replace: the user's instruction (usually derived from the transcript).
    #[serde(default)]
    pub quick_replace_instructions: Option<String>,
    /// Quick Replace: the selected/highlighted text captured at recording start.
    #[serde(default)]
    pub quick_replace_selected_text: Option<String>,
    /// Quick Replace: the rewritten output returned by the LLM.
    #[serde(default)]
    pub quick_replace_output_text: Option<String>,
    /// Quick Replace: provider used for the rewrite.
    #[serde(default)]
    pub quick_replace_provider: Option<String>,
    /// Quick Replace: model used for the rewrite.
    #[serde(default)]
    pub quick_replace_model: Option<String>,
    /// Quick Replace: duration of the rewrite call in milliseconds.
    #[serde(default)]
    pub quick_replace_duration_ms: Option<u64>,
    /// Quick Replace: clipboard text that was included as additional context (when enabled).
    #[serde(default)]
    pub quick_replace_clipboard_context: Option<String>,

    /// Quick Replace: payload sent to the rewriting LLM.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quick_replace_request_json: Option<JsonValue>,
    /// Quick Replace: payload received from the rewriting LLM.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quick_replace_response_json: Option<JsonValue>,

    /// Exact-ish payload sent to STT provider (with binary audio redacted).
    ///
    /// NOTE: For providers that use multipart/raw binary bodies, we store a JSON
    /// description of the request with placeholders for binary content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stt_request_json: Option<JsonValue>,
    /// JSON response received from STT provider (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stt_response_json: Option<JsonValue>,

    /// Payload sent to LLM provider (if LLM rewrite attempted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_request_json: Option<JsonValue>,
    /// JSON response received from LLM provider (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_response_json: Option<JsonValue>,
    /// Final result (success or error)
    pub status: RequestStatus,
    /// Error message if status is Error
    pub error_message: Option<String>,
    /// All log entries for this request
    pub entries: Vec<LogEntry>,
    /// Total duration in milliseconds
    pub total_duration_ms: Option<u64>,
    /// STT duration in milliseconds
    pub stt_duration_ms: Option<u64>,
    /// LLM duration in milliseconds
    pub llm_duration_ms: Option<u64>,

    /// Outcome of the optional LLM rewrite step.
    ///
    /// Stored as a stable string for UI display and backward-compatible persistence.
    ///
    /// Expected values:
    /// - "not_attempted"
    /// - "succeeded"
    /// - "timed_out"
    /// - "failed"
    #[serde(default)]
    pub llm_outcome: Option<String>,

    /// If `llm_outcome == "not_attempted"`, a stable reason code.
    ///
    /// Expected values:
    /// - "quiet_audio_gate"
    /// - "no_speech_detected_by_vad"
    /// - "disabled_default_profile"
    /// - "disabled_profile"
    /// - "disabled_preset"
    /// - "provider_unavailable"
    /// - "unknown"
    #[serde(default)]
    pub llm_not_attempted_reason: Option<String>,

    /// Optional details/error for the LLM rewrite step.
    ///
    /// - For `failed`: error message
    /// - For `not_attempted` + `provider_unavailable`: provider error detail
    #[serde(default)]
    pub llm_error_message: Option<String>,

    /// Intent router duration in milliseconds (when routing is enabled and actually ran).
    #[serde(default)]
    pub router_duration_ms: Option<u64>,
    /// Which router strategy was used (e.g. "embeddings" or "llm").
    #[serde(default)]
    pub router_strategy: Option<String>,
    /// Per-preset router scores (when routing ran).
    #[serde(default)]
    pub router_scores: Option<Vec<RouterPresetScore>>,

    /// Payload sent to router provider (when routing ran).
    ///
    /// For embeddings routing this may be an array of calls.
    /// For LLM routing this contains the router prompt payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub router_request_json: Option<JsonValue>,
    /// Payload received from router provider (when routing ran).
    ///
    /// For embeddings routing this may be an array of responses.
    /// For LLM routing this contains the raw router output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub router_response_json: Option<JsonValue>,

    /// Whether the STT call was treated as free-tier for pricing/cost purposes.
    #[serde(default)]
    pub stt_is_free_tier: bool,
    /// Whether the LLM call was treated as free-tier for pricing/cost purposes.
    #[serde(default)]
    pub llm_is_free_tier: bool,

    /// Estimated total STT cost for this request in USD micros.
    #[serde(default)]
    pub stt_estimated_cost_usd_micros: Option<u64>,
    /// Estimated total LLM cost for this request in USD micros.
    #[serde(default)]
    pub llm_estimated_cost_usd_micros: Option<u64>,
}

/// Status of a request
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RequestStatus {
    /// Request is in progress
    InProgress,
    /// Request completed successfully
    Success,
    /// Request failed
    Error,
    /// Request was cancelled
    Cancelled,
}

impl RequestLog {
    /// Create a new request log
    pub fn new(stt_provider: String, stt_model: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            kind: RequestKind::Transcription,
            started_at: Utc::now(),
            processing_started_at: None,
            completed_at: None,
            stt_provider,
            stt_model,
            llm_provider: None,
            llm_model: None,
            profile_id: None,
            profile_name: None,
            preset_id: None,
            preset_name: None,
            audio_duration_secs: None,
            audio_size_bytes: None,
            sample_rate: None,
            raw_transcript: None,
            formatted_transcript: None,
            rewrite_clipboard_context: None,

            quick_ask_question: None,
            quick_ask_context_text: None,
            quick_ask_clipboard_context: None,
            quick_ask_answer: None,
            quick_ask_provider: None,
            quick_ask_model: None,
            quick_ask_duration_ms: None,
            quick_ask_request_json: None,
            quick_ask_response_json: None,

            quick_replace_instructions: None,
            quick_replace_selected_text: None,
            quick_replace_output_text: None,
            quick_replace_provider: None,
            quick_replace_model: None,
            quick_replace_duration_ms: None,
            quick_replace_clipboard_context: None,
            quick_replace_request_json: None,
            quick_replace_response_json: None,
            stt_request_json: None,
            stt_response_json: None,
            llm_request_json: None,
            llm_response_json: None,
            status: RequestStatus::InProgress,
            error_message: None,
            entries: Vec::new(),
            total_duration_ms: None,
            stt_duration_ms: None,
            llm_duration_ms: None,

            llm_outcome: None,
            llm_not_attempted_reason: None,
            llm_error_message: None,

            router_duration_ms: None,
            router_strategy: None,
            router_scores: None,

            router_request_json: None,
            router_response_json: None,

            stt_is_free_tier: false,
            llm_is_free_tier: false,
            stt_estimated_cost_usd_micros: None,
            llm_estimated_cost_usd_micros: None,
        }
    }

    /// Mark the beginning of request processing (excluding recording time).
    ///
    /// This is idempotent: calling it multiple times will keep the first value.
    pub fn mark_processing_started(&mut self) {
        if self.processing_started_at.is_none() {
            self.processing_started_at = Some(Utc::now());
        }
    }

    fn compute_total_duration_ms(&self) -> Option<u64> {
        let end = self.completed_at?;
        let start = self.processing_started_at.unwrap_or(self.started_at);
        let ms = (end - start).num_milliseconds();
        if ms < 0 {
            None
        } else {
            Some(ms as u64)
        }
    }

    /// Add a log entry
    pub fn log(&mut self, level: LogLevel, message: impl Into<String>, details: Option<String>) {
        self.entries.push(LogEntry {
            timestamp: Utc::now(),
            level,
            message: message.into(),
            details,
        });
    }

    /// Log debug message
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn debug(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Debug, message, None);
    }

    /// Log info message
    pub fn info(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Info, message, None);
    }

    /// Log warning message
    pub fn warn(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Warn, message, None);
    }

    /// Log error message
    pub fn error(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Error, message, None);
    }

    /// Log error message with additional diagnostic details.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn error_with_details(&mut self, message: impl Into<String>, details: impl Into<String>) {
        self.log(LogLevel::Error, message, Some(details.into()));
    }

    /// Log with details
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn info_with_details(&mut self, message: impl Into<String>, details: impl Into<String>) {
        self.log(LogLevel::Info, message, Some(details.into()));
    }

    /// Mark request as complete with success
    pub fn complete_success(&mut self) {
        self.completed_at = Some(Utc::now());
        self.status = RequestStatus::Success;
        self.total_duration_ms = self.compute_total_duration_ms();
    }

    /// Mark request as complete with error
    pub fn complete_error(&mut self, error: impl Into<String>) {
        self.completed_at = Some(Utc::now());
        self.status = RequestStatus::Error;
        self.error_message = Some(error.into());
        self.total_duration_ms = self.compute_total_duration_ms();
    }

    /// Mark request as cancelled
    pub fn complete_cancelled(&mut self) {
        self.completed_at = Some(Utc::now());
        self.status = RequestStatus::Cancelled;
        self.total_duration_ms = self.compute_total_duration_ms();
    }
}

/// Format an error with its full causal chain.
///
/// This is especially helpful for network/TLS/DNS failures where the top-level
/// error string can be too generic (e.g., "error sending request for url").
pub fn format_error_chain(err: &(dyn Error + 'static)) -> String {
    let mut out = String::new();
    out.push_str(&err.to_string());

    let mut current: Option<&(dyn Error + 'static)> = err.source();
    let mut depth: usize = 0;
    while let Some(src) = current {
        depth += 1;
        // Keep the format stable + readable in the UI.
        out.push_str(&format!("\ncaused by ({}): {}", depth, src));
        current = src.source();
    }

    out
}

/// Thread-safe request log store
#[derive(Debug, Clone)]
pub struct RequestLogStore {
    logs: Arc<Mutex<VecDeque<RequestLog>>>,
    current: Arc<Mutex<Option<RequestLog>>>,
    retention: Arc<Mutex<RequestLogsRetentionConfig>>,
}

impl Default for RequestLogStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestLogStore {
    /// Create a new log store
    pub fn new() -> Self {
        Self::new_with_retention(RequestLogsRetentionConfig::default())
    }

    pub fn new_with_retention(retention: RequestLogsRetentionConfig) -> Self {
        // Allocate up to a modest default; VecDeque can grow, but we enforce caps on insert.
        let initial_capacity = match retention.mode {
            RequestLogsRetentionMode::Amount => retention.amount.clamp(1, HARD_MAX_LOGS),
            RequestLogsRetentionMode::Time => DEFAULT_MAX_LOGS,
        };

        Self {
            logs: Arc::new(Mutex::new(VecDeque::with_capacity(initial_capacity))),
            current: Arc::new(Mutex::new(None)),
            retention: Arc::new(Mutex::new(retention)),
        }
    }

    pub fn set_retention(&self, retention: RequestLogsRetentionConfig) {
        {
            let mut cfg = self.retention.lock().unwrap();
            *cfg = retention;
        }
        self.prune();
    }

    pub fn retention(&self) -> RequestLogsRetentionConfig {
        *self.retention.lock().unwrap()
    }

    fn prune_locked(logs: &mut VecDeque<RequestLog>, cfg: RequestLogsRetentionConfig) {
        // Time-based pruning first.
        if cfg.mode == RequestLogsRetentionMode::Time {
            if let Some(retention) = cfg.time_retention {
                let cutoff = Utc::now() - retention;
                logs.retain(|l| l.started_at >= cutoff);
            }
        }

        // Apply amount-based pruning.
        if cfg.mode == RequestLogsRetentionMode::Amount {
            let target = cfg.amount.max(1);
            while logs.len() > target {
                logs.pop_front();
            }
        }

        // Always enforce a hard cap as a safety valve.
        while logs.len() > HARD_MAX_LOGS {
            logs.pop_front();
        }
    }

    pub fn prune(&self) {
        let cfg = self.retention();
        let mut logs = self.logs.lock().unwrap();
        Self::prune_locked(&mut logs, cfg);
    }

    /// Start a new request log
    pub fn start_request(&self, stt_provider: String, stt_model: Option<String>) -> String {
        let mut current = self.current.lock().unwrap();

        // If there's an existing request, finalize it first
        if let Some(mut existing) = current.take() {
            if existing.status == RequestStatus::InProgress {
                existing.complete_cancelled();
            }
            self.store_log(existing);
        }

        let log = RequestLog::new(stt_provider, stt_model);
        let id = log.id.clone();
        *current = Some(log);
        id
    }

    /// Get the current request log for modification
    pub fn with_current<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut RequestLog) -> R,
    {
        let mut current = self.current.lock().unwrap();
        current.as_mut().map(f)
    }

    /// Complete the current request and store it
    pub fn complete_current(&self) {
        let mut current = self.current.lock().unwrap();
        if let Some(log) = current.take() {
            self.store_log(log);
        }
    }

    /// Store a completed log
    fn store_log(&self, log: RequestLog) {
        // Redact sensitive fields before keeping them in memory (and before UI export/copy).
        // Providers should also avoid logging secrets, but this is a last line of defense.
        let log = redact_request_log_json_fields(log);

        let mut logs = self.logs.lock().unwrap();
        logs.push_back(log);

        let cfg = self.retention();
        Self::prune_locked(&mut logs, cfg);
    }

    /// Get all stored logs (most recent first)
    pub fn get_logs(&self, limit: Option<usize>) -> Vec<RequestLog> {
        self.prune();

        let logs = self.logs.lock().unwrap();
        let current = self.current.lock().unwrap();

        let mut result: Vec<RequestLog> = logs
            .iter()
            .cloned()
            .map(redact_request_log_json_fields)
            .collect();

        // Add current request if exists
        if let Some(ref c) = *current {
            result.push(redact_request_log_json_fields(c.clone()));
        }

        // Reverse to get most recent first
        result.reverse();

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        result
    }

    /// Return the number of stored logs (including an in-progress current log, if any).
    pub fn count(&self) -> usize {
        self.prune();

        let logs = self.logs.lock().unwrap();
        let current = self.current.lock().unwrap();
        logs.len() + if current.is_some() { 1 } else { 0 }
    }

    /// Clear all logs
    pub fn clear(&self) {
        let mut logs = self.logs.lock().unwrap();
        logs.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_request_log_creation() {
        let log = RequestLog::new("groq".to_string(), Some("whisper-large-v3".to_string()));
        assert_eq!(log.stt_provider, "groq");
        assert_eq!(log.stt_model, Some("whisper-large-v3".to_string()));
        assert_eq!(log.status, RequestStatus::InProgress);
        assert_eq!(log.error_message, None);
    }

    #[test]
    fn test_log_entries() {
        let mut log = RequestLog::new("groq".to_string(), None);
        log.info("Recording started");
        log.debug("Audio buffer initialized");
        log.error("API call failed");

        assert_eq!(log.entries.len(), 3);
        assert_eq!(log.entries[0].level, LogLevel::Info);
        assert_eq!(log.entries[1].level, LogLevel::Debug);
        assert_eq!(log.entries[2].level, LogLevel::Error);
    }

    #[test]
    fn test_log_store() {
        let store = RequestLogStore::new();

        let id1 = store.start_request("groq".to_string(), None);
        store.with_current(|log| {
            log.info("Test message");
            log.complete_success();
        });
        store.complete_current();

        let id2 = store.start_request("openai".to_string(), None);
        store.with_current(|log| {
            log.info("Another test");
            log.complete_success();
        });
        store.complete_current();

        let logs = store.get_logs(None);
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].id, id2); // Most recent first
        assert_eq!(logs[1].id, id1);
    }

    #[test]
    fn test_redact_json_redacts_sensitive_keys() {
        let value = json!({
            "Authorization": "Bearer sk-live-should-not-appear",
            "nested": {
                "api_key": "sk-123",
                "access_token": "Bearer abc",
                "safe": "hello",
            },
            "array": [
                { "x-api-key": "rk-456" },
                "ok",
            ],
        });

        let redacted = redact_json(value);

        assert_eq!(redacted["Authorization"], "<redacted>");
        assert_eq!(redacted["nested"]["api_key"], "<redacted>");
        assert_eq!(redacted["nested"]["access_token"], "<redacted>");
        assert_eq!(redacted["nested"]["safe"], "hello");
        assert_eq!(redacted["array"][0]["x-api-key"], "<redacted>");
        assert_eq!(redacted["array"][1], "ok");
    }

    #[test]
    fn test_redact_json_redacts_token_like_strings() {
        let value = json!({
            "freeform": "Bearer abc.def.ghi",
            "openai": "sk-abcdef",
            "other": "hello",
        });

        let redacted = redact_json(value);
        assert_eq!(redacted["freeform"], "<redacted>");
        assert_eq!(redacted["openai"], "<redacted>");
        assert_eq!(redacted["other"], "hello");
    }
}
