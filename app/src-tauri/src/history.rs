use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::RwLock;
use uuid::Uuid;

/// Status of a transcription attempt in history.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HistoryStatus {
    InProgress,
    Success,
    Error,
}

impl Default for HistoryStatus {
    fn default() -> Self {
        // Existing history.json entries (pre-status) should be treated as success.
        HistoryStatus::Success
    }
}

/// A single dictation history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub text: String,
    #[serde(default)]
    pub status: HistoryStatus,
    #[serde(default)]
    pub error_message: Option<String>,
    /// Prompt profile id used for this transcription.
    ///
    /// "default" means no per-program profile matched.
    #[serde(default)]
    pub profile_id: Option<String>,
    /// Prompt profile display name used for this transcription.
    #[serde(default)]
    pub profile_name: Option<String>,
    /// STT provider used for this transcription (e.g., "groq", "openai").
    #[serde(default)]
    pub stt_provider: Option<String>,
    /// STT model used for this transcription.
    #[serde(default)]
    pub stt_model: Option<String>,
    /// LLM provider used for rewriting (if enabled).
    #[serde(default)]
    pub llm_provider: Option<String>,
    /// LLM model used for rewriting (if enabled).
    #[serde(default)]
    pub llm_model: Option<String>,

    /// Request id of the WAV recording to use for playback/rerun.
    ///
    /// - For "normal" requests this will typically equal `id` (when a recording was saved).
    /// - For reruns/retries, this should point to the original request id that owns the WAV.
    ///
    /// When `None`, no recording is known/available for this entry.
    #[serde(default)]
    pub recording_request_id: Option<String>,
}

/// Metadata about which models were used for a transcription request.
#[derive(Debug, Clone, Default)]
pub struct RequestModelInfo {
    pub stt_provider: Option<String>,
    pub stt_model: Option<String>,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
}

impl HistoryEntry {
    pub fn new(text: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            text,
            status: HistoryStatus::Success,
            error_message: None,
            profile_id: None,
            profile_name: None,
            stt_provider: None,
            stt_model: None,
            llm_provider: None,
            llm_model: None,
            recording_request_id: None,
        }
    }

    pub fn new_request_in_progress(id: String, model_info: RequestModelInfo) -> Self {
        Self {
            id,
            timestamp: Utc::now(),
            text: String::new(),
            status: HistoryStatus::InProgress,
            error_message: None,
            profile_id: model_info.profile_id,
            profile_name: model_info.profile_name,
            stt_provider: model_info.stt_provider,
            stt_model: model_info.stt_model,
            llm_provider: model_info.llm_provider,
            llm_model: model_info.llm_model,
            recording_request_id: None,
        }
    }
}

/// Storage for dictation history entries
#[derive(Debug, Serialize, Deserialize, Default)]
struct HistoryData {
    entries: Vec<HistoryEntry>,
}

/// Manages loading and saving of dictation history
pub struct HistoryStorage {
    data: RwLock<HistoryData>,
    file_path: PathBuf,
}

impl HistoryStorage {
    /// Create a new history storage with the given app data directory
    pub fn new(app_data_dir: PathBuf) -> Self {
        let file_path = app_data_dir.join("history.json");

        // Ensure the directory exists
        if let Some(parent) = file_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Load existing history or use empty
        let data = Self::load_from_file(&file_path).unwrap_or_default();

        Self {
            data: RwLock::new(data),
            file_path,
        }
    }

    /// Load history from the JSON file
    fn load_from_file(file_path: &PathBuf) -> Option<HistoryData> {
        let content = fs::read_to_string(file_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Save current history to disk
    fn save(&self) -> Result<(), String> {
        let data = self
            .data
            .read()
            .map_err(|e| format!("Failed to read history: {}", e))?;

        let content = serde_json::to_string_pretty(&*data)
            .map_err(|e| format!("Failed to serialize history: {}", e))?;

        fs::write(&self.file_path, content)
            .map_err(|e| format!("Failed to write history file: {}", e))?;

        Ok(())
    }

    /// Add a new entry to the history
    pub fn add_entry(&self, text: String, max_entries: usize) -> Result<HistoryEntry, String> {
        let entry = HistoryEntry::new(text);
        {
            let mut data = self
                .data
                .write()
                .map_err(|e| format!("Failed to write history: {}", e))?;

            // Add to the beginning (newest first)
            data.entries.insert(0, entry.clone());

            let max = max_entries.max(1);
            if data.entries.len() > max {
                data.entries.truncate(max);
            }
        }
        self.save()?;
        Ok(entry)
    }

    /// Add a new in-progress request entry with a predetermined id.
    ///
    /// This is used to show a placeholder in the History view while a transcription
    /// is running, and to keep a failed attempt visible with a retry button.
    pub fn add_request_entry(
        &self,
        request_id: String,
        model_info: RequestModelInfo,
        max_entries: usize,
    ) -> Result<HistoryEntry, String> {
        let entry = HistoryEntry::new_request_in_progress(request_id, model_info);
        {
            let mut data = self
                .data
                .write()
                .map_err(|e| format!("Failed to write history: {}", e))?;

            // Add to the beginning (newest first)
            data.entries.insert(0, entry.clone());

            let max = max_entries.max(1);
            if data.entries.len() > max {
                data.entries.truncate(max);
            }
        }
        self.save()?;
        Ok(entry)
    }

    /// Truncate history to at most `max_entries` entries.
    pub fn trim_to(&self, max_entries: usize) -> Result<(), String> {
        let max = max_entries.max(1);
        {
            let mut data = self
                .data
                .write()
                .map_err(|e| format!("Failed to write history: {}", e))?;
            if data.entries.len() > max {
                data.entries.truncate(max);
            }
        }
        self.save()
    }

    /// Delete entries older than `cutoff` (strictly earlier than cutoff).
    ///
    /// Returns the list of removed entry IDs (useful for cleaning up recordings).
    pub fn prune_older_than(&self, cutoff: DateTime<Utc>) -> Result<Vec<String>, String> {
        let mut removed: Vec<String> = Vec::new();
        let changed = {
            let mut data = self
                .data
                .write()
                .map_err(|e| format!("Failed to write history: {}", e))?;

            let before = data.entries.len();
            data.entries.retain(|entry| {
                if entry.timestamp < cutoff {
                    removed.push(entry.id.clone());
                    false
                } else {
                    true
                }
            });

            data.entries.len() != before
        };

        if changed {
            self.save()?;
        }

        Ok(removed)
    }

    /// Mark an existing request entry as successful and set the final text.
    pub fn complete_request_success(&self, request_id: &str, text: String) -> Result<(), String> {
        {
            let mut data = self
                .data
                .write()
                .map_err(|e| format!("Failed to write history: {}", e))?;

            if let Some(entry) = data.entries.iter_mut().find(|e| e.id == request_id) {
                entry.text = text;
                entry.status = HistoryStatus::Success;
                entry.error_message = None;
            } else {
                // If we somehow missed creating an in-progress entry, fall back to inserting.
                data.entries.insert(0, HistoryEntry::new_request_in_progress(request_id.to_string(), RequestModelInfo::default()));
                if let Some(entry) = data.entries.iter_mut().find(|e| e.id == request_id) {
                    entry.text = text;
                    entry.status = HistoryStatus::Success;
                    entry.error_message = None;
                }
            }
        }
        self.save()
    }

    /// Mark an existing request entry as failed with an error message.
    pub fn complete_request_error(&self, request_id: &str, error_message: String) -> Result<(), String> {
        {
            let mut data = self
                .data
                .write()
                .map_err(|e| format!("Failed to write history: {}", e))?;

            if let Some(entry) = data.entries.iter_mut().find(|e| e.id == request_id) {
                entry.status = HistoryStatus::Error;
                entry.error_message = Some(error_message);
                // Keep text as-is (likely empty). We intentionally do not delete the entry.
            } else {
                let mut entry = HistoryEntry::new_request_in_progress(request_id.to_string(), RequestModelInfo::default());
                entry.status = HistoryStatus::Error;
                entry.error_message = Some(error_message);
                data.entries.insert(0, entry);
            }
        }
        self.save()
    }

    /// Get all history entries (newest first), optionally limited
    pub fn get_all(&self, limit: Option<usize>) -> Result<Vec<HistoryEntry>, String> {
        let data = self
            .data
            .read()
            .map_err(|e| format!("Failed to read history: {}", e))?;

        let entries = match limit {
            Some(n) => data.entries.iter().take(n).cloned().collect(),
            None => data.entries.clone(),
        };

        Ok(entries)
    }

    /// Get a single history entry by request id.
    pub fn get_by_id(&self, request_id: &str) -> Result<Option<HistoryEntry>, String> {
        let data = self
            .data
            .read()
            .map_err(|e| format!("Failed to read history: {}", e))?;

        Ok(data
            .entries
            .iter()
            .find(|e| e.id == request_id)
            .cloned())
    }

    /// Update the stored profile metadata for an existing history entry.
    ///
    /// This is useful when we create an in-progress entry early, then later
    /// learn the effective profile used once transcription actually begins.
    pub fn set_request_profile(
        &self,
        request_id: &str,
        profile_id: Option<String>,
        profile_name: Option<String>,
    ) -> Result<(), String> {
        {
            let mut data = self
                .data
                .write()
                .map_err(|e| format!("Failed to write history: {}", e))?;

            if let Some(entry) = data.entries.iter_mut().find(|e| e.id == request_id) {
                entry.profile_id = profile_id;
                entry.profile_name = profile_name;
            }
        }

        self.save()
    }

    /// Update the stored recording source id for an existing history entry.
    pub fn set_request_recording_id(
        &self,
        request_id: &str,
        recording_request_id: Option<String>,
    ) -> Result<(), String> {
        {
            let mut data = self
                .data
                .write()
                .map_err(|e| format!("Failed to write history: {}", e))?;

            if let Some(entry) = data.entries.iter_mut().find(|e| e.id == request_id) {
                entry.recording_request_id = recording_request_id;
            }
        }

        self.save()
    }

    /// Query history with server-side filtering and pagination.
    ///
    /// This is primarily used by the UI to avoid transferring the entire history
    /// list across the IPC boundary on every filter keystroke.
    pub fn query_page(&self, params: HistoryPageQuery) -> Result<HistoryPageResult, String> {
        let data = self
            .data
            .read()
            .map_err(|e| format!("Failed to read history: {}", e))?;

        let entries = &data.entries;
        let total_all = entries.len();

        // Defaults match the current UI behavior.
        let filter_text = params
            .filter_text
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        let show_failed = params.show_failed.unwrap_or(true);
        let show_empty_transcript = params.show_empty_transcript.unwrap_or(false);
        let selected_stt_model_keys = params.selected_stt_model_keys.unwrap_or_default();
        let selected_llm_model_keys = params.selected_llm_model_keys.unwrap_or_default();

        let page_size_raw = params.page_size.unwrap_or(25);
        // Defensive clamp; keep page sizes reasonable for IPC payloads.
        let page_size = page_size_raw.clamp(1, 200);

        // Usage counts (across ALL history, independent of filters).
        let include_usage_counts = params.include_usage_counts.unwrap_or(true);
        let (stt_model_usage, llm_model_usage) = if include_usage_counts {
            let mut stt_counts: HashMap<String, usize> = HashMap::new();
            let mut llm_counts: HashMap<String, usize> = HashMap::new();

            for entry in entries.iter() {
                if let (Some(p), Some(m)) = (entry.stt_provider.as_ref(), entry.stt_model.as_ref()) {
                    let key = format!("{}::{}", p, m);
                    *stt_counts.entry(key).or_insert(0) += 1;
                }

                if let (Some(p), Some(m)) = (entry.llm_provider.as_ref(), entry.llm_model.as_ref()) {
                    let key = format!("{}::{}", p, m);
                    *llm_counts.entry(key).or_insert(0) += 1;
                }
            }

            let mut stt_vec: Vec<ModelUsageCount> = stt_counts
                .into_iter()
                .map(|(key, count)| ModelUsageCount { key, count })
                .collect();
            let mut llm_vec: Vec<ModelUsageCount> = llm_counts
                .into_iter()
                .map(|(key, count)| ModelUsageCount { key, count })
                .collect();

            // Sort: most-used first, stable by key for determinism.
            stt_vec.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
            llm_vec.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));

            (stt_vec, llm_vec)
        } else {
            (Vec::new(), Vec::new())
        };

        let matches_filters = |entry: &HistoryEntry| -> bool {
            // 1) Text search
            if !filter_text.is_empty() {
                let text = entry.text.to_lowercase();
                let status = match entry.status {
                    HistoryStatus::InProgress => "in_progress",
                    HistoryStatus::Success => "success",
                    HistoryStatus::Error => "error",
                };
                let err = entry
                    .error_message
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase();

                let matches = text.contains(&filter_text)
                    || status.contains(&filter_text)
                    || err.contains(&filter_text);
                if !matches {
                    return false;
                }
            }

            // 2) Show Failed
            if !show_failed && entry.status == HistoryStatus::Error {
                return false;
            }

            // 3) Show Empty transcript
            if !show_empty_transcript
                && entry.status == HistoryStatus::Success
                && entry.text.trim().is_empty()
            {
                return false;
            }

            // 4) STT model filter
            if !selected_stt_model_keys.is_empty() {
                let Some(provider) = entry.stt_provider.as_ref() else {
                    return false;
                };
                let Some(model) = entry.stt_model.as_ref() else {
                    return false;
                };
                let key = format!("{}::{}", provider, model);
                if !selected_stt_model_keys.iter().any(|k| k == &key) {
                    return false;
                }
            }

            // 5) LLM model filter
            if !selected_llm_model_keys.is_empty() {
                let Some(provider) = entry.llm_provider.as_ref() else {
                    return false;
                };
                let Some(model) = entry.llm_model.as_ref() else {
                    return false;
                };
                let key = format!("{}::{}", provider, model);
                if !selected_llm_model_keys.iter().any(|k| k == &key) {
                    return false;
                }
            }

            true
        };

        // Filter into references to avoid cloning the entire dataset.
        let mut filtered: Vec<&HistoryEntry> = Vec::new();
        filtered.reserve(entries.len().min(2048));
        for entry in entries.iter() {
            if matches_filters(entry) {
                filtered.push(entry);
            }
        }

        let total_filtered = filtered.len();
        let total_pages = ((total_filtered + page_size - 1) / page_size).max(1);

        let mut page = params.page.unwrap_or(1);
        if page < 1 {
            page = 1;
        }
        if page > total_pages {
            page = total_pages;
        }

        let start = (page - 1) * page_size;
        let end = (start + page_size).min(total_filtered);

        let items: Vec<HistoryEntry> = if start >= total_filtered {
            Vec::new()
        } else {
            filtered[start..end]
                .iter()
                .map(|e| (*e).clone())
                .collect()
        };

        Ok(HistoryPageResult {
            items,
            total_all,
            total_filtered,
            page,
            page_size,
            stt_model_usage,
            llm_model_usage,
        })
    }

    /// Return the number of stored history entries.
    pub fn count(&self) -> Result<usize, String> {
        let data = self
            .data
            .read()
            .map_err(|e| format!("Failed to read history: {}", e))?;
        Ok(data.entries.len())
    }

    /// Best-effort history file size on disk (bytes).
    pub fn file_size_bytes(&self) -> u64 {
        std::fs::metadata(&self.file_path)
            .map(|m| m.len())
            .unwrap_or(0)
    }

    /// Delete an entry by ID
    pub fn delete(&self, id: &str) -> Result<bool, String> {
        let deleted = {
            let mut data = self
                .data
                .write()
                .map_err(|e| format!("Failed to write history: {}", e))?;

            let initial_len = data.entries.len();
            data.entries.retain(|e| e.id != id);
            data.entries.len() < initial_len
        };

        if deleted {
            self.save()?;
        }

        Ok(deleted)
    }

    /// Delete multiple entries by ID in a single save.
    ///
    /// Returns the number of entries removed.
    pub fn delete_many(&self, ids: &HashSet<String>) -> Result<usize, String> {
        if ids.is_empty() {
            return Ok(0);
        }

        let deleted = {
            let mut data = self
                .data
                .write()
                .map_err(|e| format!("Failed to write history: {}", e))?;

            let initial_len = data.entries.len();
            data.entries.retain(|e| !ids.contains(&e.id));
            initial_len.saturating_sub(data.entries.len())
        };

        if deleted > 0 {
            self.save()?;
        }

        Ok(deleted)
    }

    /// Clear `recording_request_id` for any entries pointing at a given recording source.
    ///
    /// Returns the number of entries updated.
    pub fn clear_recording_request_id_for_source(
        &self,
        recording_request_id: &str,
    ) -> Result<usize, String> {
        let mut updated = 0usize;

        {
            let mut data = self
                .data
                .write()
                .map_err(|e| format!("Failed to write history: {}", e))?;

            for entry in data.entries.iter_mut() {
                if entry.recording_request_id.as_deref() == Some(recording_request_id) {
                    entry.recording_request_id = None;
                    updated += 1;
                }
            }
        }

        if updated > 0 {
            self.save()?;
        }

        Ok(updated)
    }

    /// Clear all history
    pub fn clear(&self) -> Result<(), String> {
        {
            let mut data = self
                .data
                .write()
                .map_err(|e| format!("Failed to write history: {}", e))?;
            data.entries.clear();
        }
        self.save()
    }
}

// ============================================================================
// Server-side paging/filtering structs (UI API)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageCount {
    pub key: String,
    pub count: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPageQuery {
    pub filter_text: Option<String>,
    pub show_failed: Option<bool>,
    pub show_empty_transcript: Option<bool>,
    pub selected_stt_model_keys: Option<Vec<String>>,
    pub selected_llm_model_keys: Option<Vec<String>>,

    /// 1-based page index.
    pub page: Option<usize>,
    pub page_size: Option<usize>,

    /// When true, include per-model usage counts in the response.
    pub include_usage_counts: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPageResult {
    pub items: Vec<HistoryEntry>,
    pub total_all: usize,
    pub total_filtered: usize,
    pub page: usize,
    pub page_size: usize,
    pub stt_model_usage: Vec<ModelUsageCount>,
    pub llm_model_usage: Vec<ModelUsageCount>,
}
