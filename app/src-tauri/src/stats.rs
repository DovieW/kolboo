//! Persistent usage/cost stats ledger.
//!
//! This is intentionally separate from `request_log`:
//! - request logs are in-memory and meant for debugging.
//! - stats are persisted (for usage analytics / cost reporting).

use chrono::{DateTime, Duration as ChronoDuration, Timelike, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::fs;
use std::io::{BufRead, BufReader};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Instant;
use uuid::Uuid;

use crate::app_paths::ensure_dir;
use crate::cost::anthropic as anthropic_cost;
use crate::cost::aquavoice as aquavoice_cost;
use crate::cost::assemblyai as assemblyai_cost;
use crate::cost::deepgram as deepgram_cost;
use crate::cost::fireworks as fireworks_cost;
use crate::cost::gemini as gemini_cost;
use crate::cost::groq as groq_cost;
use crate::cost::openai as openai_cost;
use crate::cost::speechmatics as speechmatics_cost;
use crate::events;
use crate::request_log::RequestLogStore;
use tauri::AppHandle;
use tauri::{Emitter, Manager};

#[derive(Debug, Clone)]
struct StatsQueryCacheEntry {
    revision: u64,
    value: JsonValue,
}

#[derive(Debug, Default)]
struct StatsQueryCacheState {
    cost_summary: std::collections::HashMap<String, StatsQueryCacheEntry>,
    cost_by_provider: std::collections::HashMap<String, StatsQueryCacheEntry>,
}

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
        match provider {
            "cerebras" => crate::get_setting_from_store(app, "cerebras_free_tier", true),
            "groq" => crate::get_setting_from_store(app, "groq_free_tier", true),
            "elevenlabs" => crate::get_setting_from_store(app, "elevenlabs_free_tier", true),
            "cohere" => crate::get_setting_from_store(app, "cohere_free_tier", true),
            "assemblyai" => crate::get_setting_from_store(app, "assemblyai_free_tier", true),
            "speechmatics" => crate::get_setting_from_store(app, "speechmatics_free_tier", true),
            _ => false,
        }
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
#[derive(Debug)]
pub struct StatsStore {
    dir: PathBuf,
    writer_state: Arc<StdMutex<StatsWriterState>>,
    revision: AtomicU64,
    query_cache: Arc<StdMutex<StatsQueryCacheState>>,
}

impl Clone for StatsStore {
    fn clone(&self) -> Self {
        Self {
            dir: self.dir.clone(),
            writer_state: self.writer_state.clone(),
            revision: AtomicU64::new(self.revision.load(Ordering::Relaxed)),
            query_cache: self.query_cache.clone(),
        }
    }
}

#[derive(Debug)]
struct StatsWriterState {
    current_date: Option<String>,
    current_path: Option<PathBuf>,
    writer: Option<BufWriter<fs::File>>,
    pending_appends: u32,
    last_flush_at: Instant,

    // Best-effort on-disk hourly index for cost events.
    // This makes stats queries fast immediately after restart.
    index_date: Option<String>,
    index_day: Option<cost_index::CostIndexDay>,
    index_dirty: bool,
}

impl StatsStore {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let dir = app_data_dir.join("stats");
        if let Err(e) = ensure_dir(&dir) {
            log::warn!("Failed to create stats dir {:?}: {}", dir, e);
        }
        Self {
            dir,
            writer_state: Arc::new(StdMutex::new(StatsWriterState {
                current_date: None,
                current_path: None,
                writer: None,
                pending_appends: 0,
                last_flush_at: Instant::now(),

                index_date: None,
                index_day: None,
                index_dirty: false,
            })),
            revision: AtomicU64::new(0),
            query_cache: Arc::new(StdMutex::new(StatsQueryCacheState::default())),
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }

    fn bump_revision(&self) {
        self.revision.fetch_add(1, Ordering::Relaxed);
    }

    pub fn cache_get_cost_summary<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let rev = self.revision();
        let lock = self.query_cache.lock().ok()?;
        let entry = lock.cost_summary.get(key)?;
        if entry.revision != rev {
            return None;
        }
        serde_json::from_value(entry.value.clone()).ok()
    }

    pub fn cache_put_cost_summary<T: Serialize>(&self, key: String, value: &T) {
        let Ok(v) = serde_json::to_value(value) else {
            return;
        };
        let rev = self.revision();
        if let Ok(mut lock) = self.query_cache.lock() {
            // Keep the cache bounded.
            if lock.cost_summary.len() >= 64 {
                lock.cost_summary.clear();
            }
            lock.cost_summary.insert(
                key,
                StatsQueryCacheEntry {
                    revision: rev,
                    value: v,
                },
            );
        }
    }

    pub fn cache_get_cost_by_provider<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let rev = self.revision();
        let lock = self.query_cache.lock().ok()?;
        let entry = lock.cost_by_provider.get(key)?;
        if entry.revision != rev {
            return None;
        }
        serde_json::from_value(entry.value.clone()).ok()
    }

    pub fn cache_put_cost_by_provider<T: Serialize>(&self, key: String, value: &T) {
        let Ok(v) = serde_json::to_value(value) else {
            return;
        };
        let rev = self.revision();
        if let Ok(mut lock) = self.query_cache.lock() {
            if lock.cost_by_provider.len() >= 64 {
                lock.cost_by_provider.clear();
            }
            lock.cost_by_provider.insert(
                key,
                StatsQueryCacheEntry {
                    revision: rev,
                    value: v,
                },
            );
        }
    }

    pub fn append_cost_event(&self, event: &CostEvent) -> Result<(), String> {
        ensure_dir(&self.dir)?;

        let date = event.created_at.format("%Y-%m-%d").to_string();
        let file_path = self.dir.join(format!("cost-events-{}.jsonl", date));

        let mut st = self
            .writer_state
            .lock()
            .map_err(|_| "StatsStore writer_state lock poisoned".to_string())?;

        let needs_rotate = st
            .current_date
            .as_deref()
            .map(|d| d != date.as_str())
            .unwrap_or(true);

        if needs_rotate {
            // Best-effort flush old writer before rotating.
            if let Some(ref mut w) = st.writer {
                let _ = w.flush();
            }

            // Best-effort flush old index before rotating.
            if st.index_dirty {
                if let (Some(idx), Some(idx_date)) = (st.index_day.as_ref(), st.index_date.as_ref())
                {
                    if let Err(e) = cost_index::save_day(&self.dir, idx_date, idx) {
                        log::warn!(
                            "Failed to save cost index for {}: {} (will rebuild later)",
                            idx_date,
                            e
                        );
                    }
                }
                st.index_dirty = false;
            }

            let file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)
                .map_err(|e| format!("Failed to open stats file {:?}: {}", file_path, e))?;

            st.writer = Some(BufWriter::new(file));
            st.current_date = Some(date);
            st.current_path = Some(file_path);
            st.pending_appends = 0;

            // Load (or rebuild) the index for the new shard.
            let new_date = st
                .current_date
                .clone()
                .unwrap_or_else(|| event.created_at.format("%Y-%m-%d").to_string());
            match cost_index::load_or_rebuild_day(&self.dir, &new_date) {
                Ok(day) => {
                    st.index_date = Some(new_date);
                    st.index_day = Some(day);
                    st.index_dirty = false;
                }
                Err(e) => {
                    log::warn!(
                        "Failed to load/rebuild cost index for {}: {} (stats queries will fall back)",
                        new_date,
                        e
                    );
                    st.index_date = Some(new_date);
                    st.index_day = None;
                    st.index_dirty = false;
                }
            }
        }

        let Some(ref mut writer) = st.writer else {
            return Err("StatsStore writer missing".to_string());
        };

        serde_json::to_writer(&mut *writer, event).map_err(|e| e.to_string())?;
        writer.write_all(b"\n").map_err(|e| e.to_string())?;
        st.pending_appends = st.pending_appends.saturating_add(1);

        // Best-effort: update the hourly index for this day.
        let idx_date = st.index_date.clone();
        let mut index_updated = false;
        if let Some(ref mut idx) = st.index_day {
            let event_date = event.created_at.format("%Y-%m-%d").to_string();
            if idx_date.as_deref() == Some(event_date.as_str()) {
                idx.apply_event(event);
                index_updated = true;
            }
        }
        if index_updated {
            st.index_dirty = true;
        }

        // Any append means stats queries are stale.
        self.bump_revision();
        Ok(())
    }

    /// Flush any buffered stats writes to disk.
    ///
    /// This does *not* fsync; it only ensures other readers can see appended lines.
    pub fn flush(&self) -> Result<(), String> {
        let mut st = self
            .writer_state
            .lock()
            .map_err(|_| "StatsStore writer_state lock poisoned".to_string())?;

        // Skip redundant flushes if nothing changed.
        if st.pending_appends == 0 {
            return Ok(());
        }

        if let Some(ref mut writer) = st.writer {
            writer.flush().map_err(|e| e.to_string())?;
        }

        // Best-effort persist index alongside the shard flush.
        if st.index_dirty {
            if let (Some(idx), Some(idx_date)) = (st.index_day.as_ref(), st.index_date.as_ref()) {
                if let Err(e) = cost_index::save_day(&self.dir, idx_date, idx) {
                    // Do not fail the app for an index write; we can rebuild it later.
                    log::warn!(
                        "Failed to save cost index for {}: {} (will rebuild later)",
                        idx_date,
                        e
                    );
                } else {
                    st.index_dirty = false;
                }
            }
        }

        st.pending_appends = 0;
        st.last_flush_at = Instant::now();
        Ok(())
    }

    fn current_open_path(&self) -> Option<PathBuf> {
        self.writer_state
            .lock()
            .ok()
            .and_then(|st| st.current_path.clone())
    }

    pub fn prune(&self, cfg: StatsRetentionConfig) -> Result<(), String> {
        ensure_dir(&self.dir)?;

        // Avoid deleting the currently-open shard file.
        let open_path = self.current_open_path();

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

                if open_path.as_ref().is_some_and(|p| p == &path) {
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

                        // Also delete the corresponding index (best-effort).
                        let idx_path = cost_index::index_path_for_date(&self.dir, date_part);
                        let _ = fs::remove_file(idx_path);
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

            if open_path.as_ref().is_some_and(|p| p == &path) {
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

                // If we deleted a shard file, also delete its index (best-effort).
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    if name.starts_with("cost-events-") && name.ends_with(".jsonl") {
                        let date_part = name
                            .trim_start_matches("cost-events-")
                            .trim_end_matches(".jsonl");
                        let idx_path = cost_index::index_path_for_date(&self.dir, date_part);
                        let _ = fs::remove_file(idx_path);
                    }
                }
            }
        }

        Ok(())
    }

    /// Aggregate cost events using the persisted hourly index when possible.
    ///
    /// This is used by the stats commands to avoid re-scanning all JSONL lines
    /// on every query, while keeping results exact.
    pub fn aggregate_cost_events(
        &self,
        cutoff: Option<DateTime<Utc>>,
        kind_filter: Option<CostKind>,
        selected_stt_model_keys: Option<&std::collections::HashSet<String>>,
        selected_llm_model_keys: Option<&std::collections::HashSet<String>>,
        exclude_free_tier: bool,
    ) -> Result<AggregatedCostResult, String> {
        let mut out = AggregatedCostResult::default();

        if !self.dir.exists() {
            return Ok(out);
        }

        fn apply_event_filtered(
            out: &mut AggregatedCostResult,
            ev: &CostEvent,
            cutoff: Option<DateTime<Utc>>,
            kind_filter: Option<CostKind>,
            selected_stt_model_keys: Option<&std::collections::HashSet<String>>,
            selected_llm_model_keys: Option<&std::collections::HashSet<String>>,
            exclude_free_tier: bool,
        ) {
            if let Some(cut) = cutoff {
                if ev.created_at < cut {
                    return;
                }
            }

            if let Some(kind_filter) = kind_filter {
                if ev.kind != kind_filter {
                    return;
                }
            }

            if exclude_free_tier && ev.is_free_tier {
                return;
            }

            let keys_to_apply = match ev.kind {
                CostKind::Stt => selected_stt_model_keys,
                CostKind::Llm => selected_llm_model_keys,
            };

            if let Some(keys) = keys_to_apply {
                let Some(model) = ev.model.as_deref() else {
                    return;
                };
                let key = format!("{}::{}", ev.provider, model);
                if !keys.contains(&key) {
                    return;
                }
            }

            out.events_total = out.events_total.saturating_add(1);
            out.earliest_included_at = match out.earliest_included_at {
                None => Some(ev.created_at),
                Some(t) => Some(std::cmp::min(t, ev.created_at)),
            };
            out.latest_included_at = match out.latest_included_at {
                None => Some(ev.created_at),
                Some(t) => Some(std::cmp::max(t, ev.created_at)),
            };

            // Provider totals always include events_total, regardless of cost.
            let entry = out
                .by_provider
                .entry(ev.provider.clone())
                .or_insert((0u128, 0u64, 0u64));
            entry.1 = entry.1.saturating_add(1);

            if let Some(micros) = ev.estimated_cost_usd_micros {
                out.events_with_cost = out.events_with_cost.saturating_add(1);
                out.total_usd_micros = out.total_usd_micros.saturating_add(micros as u128);
                entry.0 = entry.0.saturating_add(micros as u128);
                entry.2 = entry.2.saturating_add(1);
            }
        }

        // If cutoff exists, we can avoid double-counting by treating the cutoff hour
        // as the only "partial" interval that needs exact JSONL scanning.
        let cutoff_date = cutoff.map(|t| t.date_naive());
        let cutoff_hour = cutoff.map(|t| t.hour());

        // 1) Use index for all available shard dates.
        // For cutoff day, include only hours strictly after the cutoff hour.
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

            let date_part = name
                .trim_start_matches("cost-events-")
                .trim_end_matches(".jsonl");
            let Ok(date) = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d") else {
                continue;
            };

            // If cutoff exists and this whole day is strictly before it, skip.
            if let Some(cut_date) = cutoff_date {
                if date < cut_date {
                    continue;
                }
            }

            let day_res = cost_index::load_or_rebuild_day(&self.dir, date_part);
            let day = match day_res {
                Ok(d) => d,
                Err(_) => {
                    // If index can't be loaded/rebuilt, fall back to scanning this shard.
                    // This keeps correctness and allows recovery even if index logic fails.
                    let file = match fs::File::open(&path) {
                        Ok(f) => f,
                        Err(_) => continue,
                    };
                    let reader = BufReader::new(file);
                    for line in reader.lines() {
                        let Ok(line) = line else { continue };
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        let Ok(ev) = serde_json::from_str::<CostEvent>(line) else {
                            continue;
                        };

                        // Keep the cutoff hour exclusively scanned in the dedicated pass below,
                        // so we can apply the minute/second cutoff precisely without double-counting.
                        if let (Some(cut_date), Some(cut_hour)) = (cutoff_date, cutoff_hour) {
                            if ev.created_at.date_naive() == cut_date
                                && (ev.created_at.hour() as u32) <= cut_hour
                            {
                                continue;
                            }
                        }

                        apply_event_filtered(
                            &mut out,
                            &ev,
                            cutoff,
                            kind_filter,
                            selected_stt_model_keys,
                            selected_llm_model_keys,
                            exclude_free_tier,
                        );
                    }
                    continue;
                }
            };

            let start_hour_exclusive: Option<u32> = match (cutoff_date, cutoff_hour) {
                (Some(cut_date), Some(h)) if date == cut_date => Some(h),
                _ => None,
            };

            day.sum_into(
                &mut out,
                cost_index::CostAggFilters {
                    cutoff,
                    kind_filter,
                    selected_stt_model_keys,
                    selected_llm_model_keys,
                    exclude_free_tier,
                },
                start_hour_exclusive,
            );
        }

        // 2) Exact scan for the cutoff hour (only), to exclude events earlier than the cutoff.
        // This is needed because the index aggregates by hour.
        if let (Some(cut), Some(cut_date), Some(cut_hour)) = (cutoff, cutoff_date, cutoff_hour) {
            let shard_path = self
                .dir
                .join(format!("cost-events-{}.jsonl", cut_date.format("%Y-%m-%d")));
            if shard_path.exists() {
                let file = fs::File::open(&shard_path).map_err(|e| e.to_string())?;
                let reader = BufReader::new(file);
                for line in reader.lines() {
                    let line = match line {
                        Ok(l) => l,
                        Err(_) => continue,
                    };
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    let ev: CostEvent = match serde_json::from_str(line) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    if ev.created_at < cut {
                        continue;
                    }

                    if ev.created_at.date_naive() != cut_date {
                        continue;
                    }

                    if ev.created_at.hour() != cut_hour {
                        continue;
                    }

                    apply_event_filtered(
                        &mut out,
                        &ev,
                        cutoff,
                        kind_filter,
                        selected_stt_model_keys,
                        selected_llm_model_keys,
                        exclude_free_tier,
                    );
                }
            }
        }

        Ok(out)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[derive(Debug, Default, Clone)]
pub struct AggregatedCostResult {
    pub total_usd_micros: u128,
    pub events_total: u64,
    pub events_with_cost: u64,
    pub earliest_included_at: Option<DateTime<Utc>>,
    pub latest_included_at: Option<DateTime<Utc>>,
    // provider -> (total_usd_micros, events_total, events_with_cost)
    pub by_provider: std::collections::HashMap<String, (u128, u64, u64)>,
}

mod cost_index {
    use super::{AggregatedCostResult, CostEvent, CostKind};
    use chrono::{DateTime, Timelike, Utc};
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::fs;
    use std::hash::{Hash, Hasher};
    use std::io::{BufRead, BufReader};
    use std::path::{Path, PathBuf};

    const COST_INDEX_VERSION: u32 = 1;

    pub fn index_path_for_date(dir: &Path, date: &str) -> PathBuf {
        dir.join(format!("cost-index-{}.json", date))
    }

    #[derive(Debug, Clone, Copy)]
    pub struct CostAggFilters<'a> {
        pub cutoff: Option<DateTime<Utc>>,
        pub kind_filter: Option<CostKind>,
        pub selected_stt_model_keys: Option<&'a std::collections::HashSet<String>>,
        pub selected_llm_model_keys: Option<&'a std::collections::HashSet<String>>,
        pub exclude_free_tier: bool,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct CostIndexDayDisk {
        version: u32,
        date: String,
        hours: Vec<CostIndexHourDisk>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct CostIndexHourDisk {
        hour: u8,
        buckets: Vec<CostIndexBucketDisk>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct CostIndexBucketDisk {
        provider: String,
        kind: CostKind,
        model: Option<String>,
        is_free_tier: bool,
        total_usd_micros: u64,
        events_total: u64,
        events_with_cost: u64,
        earliest_at: Option<DateTime<Utc>>,
        latest_at: Option<DateTime<Utc>>,
    }

    #[derive(Debug, Clone, Eq)]
    struct CostIndexKey {
        provider: String,
        kind: CostKind,
        model: Option<String>,
        is_free_tier: bool,
    }

    fn kind_ord(kind: CostKind) -> u8 {
        match kind {
            CostKind::Stt => 0,
            CostKind::Llm => 1,
        }
    }

    impl PartialEq for CostIndexKey {
        fn eq(&self, other: &Self) -> bool {
            self.provider == other.provider
                && self.kind == other.kind
                && self.model == other.model
                && self.is_free_tier == other.is_free_tier
        }
    }

    impl Hash for CostIndexKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.provider.hash(state);
            kind_ord(self.kind).hash(state);
            self.model.hash(state);
            self.is_free_tier.hash(state);
        }
    }

    #[derive(Debug, Clone, Default)]
    struct CostIndexAgg {
        total_usd_micros: u128,
        events_total: u64,
        events_with_cost: u64,
        earliest_at: Option<DateTime<Utc>>,
        latest_at: Option<DateTime<Utc>>,
    }

    #[derive(Debug, Clone)]
    pub struct CostIndexDay {
        pub date: String,
        hours: Vec<HashMap<CostIndexKey, CostIndexAgg>>,
    }

    impl CostIndexDay {
        pub fn new(date: String) -> Self {
            let mut hours = Vec::with_capacity(24);
            for _ in 0..24 {
                hours.push(HashMap::new());
            }
            Self { date, hours }
        }

        pub fn apply_event(&mut self, ev: &CostEvent) {
            let hour = (ev.created_at.hour().min(23)) as usize;
            let key = CostIndexKey {
                provider: ev.provider.clone(),
                kind: ev.kind,
                model: ev.model.clone(),
                is_free_tier: ev.is_free_tier,
            };
            let entry = self.hours[hour].entry(key).or_default();
            entry.events_total = entry.events_total.saturating_add(1);
            entry.earliest_at = match entry.earliest_at {
                None => Some(ev.created_at),
                Some(t) => Some(std::cmp::min(t, ev.created_at)),
            };
            entry.latest_at = match entry.latest_at {
                None => Some(ev.created_at),
                Some(t) => Some(std::cmp::max(t, ev.created_at)),
            };

            if let Some(micros) = ev.estimated_cost_usd_micros {
                entry.events_with_cost = entry.events_with_cost.saturating_add(1);
                entry.total_usd_micros = entry.total_usd_micros.saturating_add(micros as u128);
            }
        }

        pub fn sum_into(
            &self,
            out: &mut AggregatedCostResult,
            filters: CostAggFilters<'_>,
            start_hour_exclusive: Option<u32>,
        ) {
            for (hour, map) in self.hours.iter().enumerate() {
                if let Some(h) = start_hour_exclusive {
                    if (hour as u32) <= h {
                        continue;
                    }
                }

                for (k, agg) in map {
                    if filters.exclude_free_tier && k.is_free_tier {
                        continue;
                    }

                    if let Some(kind_filter) = filters.kind_filter {
                        if k.kind != kind_filter {
                            continue;
                        }
                    }

                    // Model filters match command behavior: require model to exist.
                    let keys_to_apply = match k.kind {
                        CostKind::Stt => filters.selected_stt_model_keys,
                        CostKind::Llm => filters.selected_llm_model_keys,
                    };
                    if let Some(keys) = keys_to_apply {
                        let Some(model) = k.model.as_deref() else {
                            continue;
                        };
                        let key = format!("{}::{}", k.provider, model);
                        if !keys.contains(&key) {
                            continue;
                        }
                    }

                    out.events_total = out.events_total.saturating_add(agg.events_total);
                    out.events_with_cost =
                        out.events_with_cost.saturating_add(agg.events_with_cost);
                    out.total_usd_micros =
                        out.total_usd_micros.saturating_add(agg.total_usd_micros);

                    if let Some(t) = agg.earliest_at {
                        out.earliest_included_at = Some(match out.earliest_included_at {
                            None => t,
                            Some(cur) => std::cmp::min(cur, t),
                        });
                    }
                    if let Some(t) = agg.latest_at {
                        out.latest_included_at = Some(match out.latest_included_at {
                            None => t,
                            Some(cur) => std::cmp::max(cur, t),
                        });
                    }

                    // Provider totals.
                    let entry = out
                        .by_provider
                        .entry(k.provider.clone())
                        .or_insert((0u128, 0u64, 0u64));
                    entry.1 = entry.1.saturating_add(agg.events_total);
                    entry.2 = entry.2.saturating_add(agg.events_with_cost);
                    entry.0 = entry.0.saturating_add(agg.total_usd_micros);
                }
            }
        }
    }

    pub fn save_day(dir: &Path, date: &str, day: &CostIndexDay) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = index_path_for_date(dir, date);

        let mut hours: Vec<CostIndexHourDisk> = Vec::with_capacity(24);
        for (hour, map) in day.hours.iter().enumerate() {
            let mut buckets: Vec<CostIndexBucketDisk> = Vec::with_capacity(map.len());
            for (k, agg) in map {
                buckets.push(CostIndexBucketDisk {
                    provider: k.provider.clone(),
                    kind: k.kind,
                    model: k.model.clone(),
                    is_free_tier: k.is_free_tier,
                    total_usd_micros: (agg.total_usd_micros.min(u128::from(u64::MAX))) as u64,
                    events_total: agg.events_total,
                    events_with_cost: agg.events_with_cost,
                    earliest_at: agg.earliest_at,
                    latest_at: agg.latest_at,
                });
            }
            // Stable-ish ordering helps diffs and debugging.
            buckets.sort_by(|a, b| {
                a.provider
                    .cmp(&b.provider)
                    .then_with(|| kind_ord(a.kind).cmp(&kind_ord(b.kind)))
                    .then_with(|| a.model.cmp(&b.model))
                    .then_with(|| a.is_free_tier.cmp(&b.is_free_tier))
            });

            hours.push(CostIndexHourDisk {
                hour: hour as u8,
                buckets,
            });
        }

        let disk = CostIndexDayDisk {
            version: COST_INDEX_VERSION,
            date: day.date.clone(),
            hours,
        };

        let bytes = serde_json::to_vec(&disk).map_err(|e| e.to_string())?;
        fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_day_from_disk(dir: &Path, date: &str) -> Result<CostIndexDay, String> {
        let path = index_path_for_date(dir, date);
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        let disk: CostIndexDayDisk = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if disk.version != COST_INDEX_VERSION {
            return Err(format!("Unsupported cost index version: {}", disk.version));
        }

        let mut day = CostIndexDay::new(disk.date);
        for h in disk.hours {
            let idx = (h.hour.min(23)) as usize;
            let map = &mut day.hours[idx];
            for b in h.buckets {
                map.insert(
                    CostIndexKey {
                        provider: b.provider,
                        kind: b.kind,
                        model: b.model,
                        is_free_tier: b.is_free_tier,
                    },
                    CostIndexAgg {
                        total_usd_micros: b.total_usd_micros as u128,
                        events_total: b.events_total,
                        events_with_cost: b.events_with_cost,
                        earliest_at: b.earliest_at,
                        latest_at: b.latest_at,
                    },
                );
            }
        }

        Ok(day)
    }

    fn rebuild_day_from_shard(dir: &Path, date: &str) -> Result<CostIndexDay, String> {
        let shard_path = dir.join(format!("cost-events-{}.jsonl", date));
        if !shard_path.exists() {
            return Ok(CostIndexDay::new(date.to_string()));
        }

        let file = fs::File::open(&shard_path).map_err(|e| e.to_string())?;
        let reader = BufReader::new(file);
        let mut day = CostIndexDay::new(date.to_string());

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let ev: CostEvent = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            day.apply_event(&ev);
        }

        Ok(day)
    }

    pub fn load_or_rebuild_day(dir: &Path, date: &str) -> Result<CostIndexDay, String> {
        let idx_path = index_path_for_date(dir, date);

        if idx_path.exists() {
            match load_day_from_disk(dir, date) {
                Ok(day) => return Ok(day),
                Err(e) => {
                    // Fall through to rebuild.
                    log::warn!(
                        "Cost index file {:?} is unreadable: {} (rebuilding)",
                        idx_path,
                        e
                    );
                }
            }
        }

        let day = rebuild_day_from_shard(dir, date)?;
        // Best-effort persist; if it fails we can still return the in-memory version.
        let _ = save_day(dir, date, &day);
        Ok(day)
    }
}

#[cfg(desktop)]
pub fn read_stats_retention_config(app: &tauri::AppHandle) -> StatsRetentionConfig {
    let unit: String = crate::get_setting_from_store(app, "stats_retention_unit", "days".into());
    let value: f64 = crate::get_setting_from_store(app, "stats_retention_value", 30.0f64);
    let max_bytes: u64 =
        crate::get_setting_from_store(app, "stats_retention_max_bytes", 50_000_000u64);

    let value = if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    };

    let time_retention = if value == 0.0 {
        None
    } else if unit == "hours" {
        Some(ChronoDuration::milliseconds(
            (value * 3600.0 * 1000.0) as i64,
        ))
    } else {
        // Default: days
        Some(ChronoDuration::milliseconds(
            (value * 24.0 * 3600.0 * 1000.0) as i64,
        ))
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
        let prompt = usage
            .get("prompt_tokens")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
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
    let input_tokens = usage
        .get("input_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
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
    let usage = v.get("usageMetadata").or_else(|| v.get("usage_metadata"))?;

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
pub fn parse_anthropic_usage_from_response_json(
    v: &JsonValue,
) -> Option<anthropic_cost::AnthropicUsage> {
    let usage = v.get("usage")?;

    let input_tokens = usage
        .get("input_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

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
        let split_sum =
            cache_creation_5m_input_tokens.saturating_add(cache_creation_1h_input_tokens);
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
    log::info!(
        "emit_cost_events_for_current_request called with status {:?}",
        status
    );

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
                log.quick_ask_provider
                    .clone()
                    .or_else(|| log.llm_provider.clone()),
                log.quick_ask_model
                    .clone()
                    .or_else(|| log.llm_model.clone()),
                log.quick_ask_response_json
                    .clone()
                    .or_else(|| log.llm_response_json.clone()),
            ),
            crate::request_log::RequestKind::QuickReplace => (
                log.quick_replace_provider
                    .clone()
                    .or_else(|| log.llm_provider.clone()),
                log.quick_replace_model
                    .clone()
                    .or_else(|| log.llm_model.clone()),
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
            if let (Some(model), Some(resp)) =
                (ev.model.as_deref(), inputs.stt_response_json.as_ref())
            {
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
                    if let Some(micros) =
                        openai_cost::estimate_transcription_cost_from_audio_secs(model, secs)
                    {
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
                    if let Some(micros) = groq_cost::estimate_stt_cost_from_audio_secs(model, secs)
                    {
                        ev.estimated_cost_usd_micros = Some(micros);
                    }
                }
            }
        }

        if inputs.stt_provider == "deepgram" && ev.estimated_cost_usd_micros.is_none() {
            if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
                if let Some(micros) = deepgram_cost::estimate_stt_cost_from_audio_secs(model, secs)
                {
                    ev.estimated_cost_usd_micros = Some(micros);
                }
            }
        }

        if inputs.stt_provider == "aquavoice" && ev.estimated_cost_usd_micros.is_none() {
            if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
                if let Some(micros) = aquavoice_cost::estimate_stt_cost_from_audio_secs(model, secs)
                {
                    ev.estimated_cost_usd_micros = Some(micros);
                }
            }
        }

        if inputs.stt_provider == "assemblyai" && ev.estimated_cost_usd_micros.is_none() {
            if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
                if let Some(micros) =
                    assemblyai_cost::estimate_stt_cost_from_audio_secs(model, secs)
                {
                    ev.estimated_cost_usd_micros = Some(micros);
                }
            }
        }

        if inputs.stt_provider == "speechmatics" {
            // Even when marked free-tier, still estimate list-price cost so users can
            // optionally include free-tier calls in Stats.
            if ev.estimated_cost_usd_micros.is_none() {
                if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
                    if let Some(micros) =
                        speechmatics_cost::estimate_stt_cost_from_audio_secs(model, secs)
                    {
                        ev.estimated_cost_usd_micros = Some(micros);
                    }
                }
            }
        }

        if inputs.stt_provider == "fireworks" && ev.estimated_cost_usd_micros.is_none() {
            if let (Some(model), Some(secs)) = (ev.model.as_deref(), audio_secs) {
                if let Some(micros) = fireworks_cost::estimate_stt_cost_from_audio_secs(model, secs)
                {
                    ev.estimated_cost_usd_micros = Some(micros);
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
            log::info!(
                "Successfully wrote STT cost event for request {}, cost: {:?} micros",
                inputs.request_id,
                ev.estimated_cost_usd_micros
            );
        }
    }

    // LLM cost event (only if an LLM provider/model is set)
    if let (Some(llm_provider), Some(llm_model)) =
        (inputs.llm_provider.as_deref(), inputs.llm_model.as_deref())
    {
        let mut ev = CostEvent::new(
            inputs.request_id.clone(),
            CostKind::Llm,
            llm_provider.to_string(),
            Some(llm_model.to_string()),
            status,
        );

        ev.is_free_tier = is_free_tier_call(app, ev.provider.as_str());

        if llm_provider == "openai" {
            if let (Some(model), Some(resp)) =
                (ev.model.as_deref(), inputs.llm_response_json.as_ref())
            {
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
            if let (Some(model), Some(resp)) =
                (ev.model.as_deref(), inputs.llm_response_json.as_ref())
            {
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
            if let (Some(model), Some(resp)) =
                (ev.model.as_deref(), inputs.llm_response_json.as_ref())
            {
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
            if let (Some(model), Some(resp)) =
                (ev.model.as_deref(), inputs.llm_response_json.as_ref())
            {
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
            if let (Some(model), Some(resp)) =
                (ev.model.as_deref(), inputs.llm_response_json.as_ref())
            {
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
    // Flush once per request so the Stats UI can read newly-appended lines immediately,
    // but avoid paying a flush cost for every individual cost event.
    if let Err(e) = stats_store.flush() {
        log::warn!("Failed to flush stats writer: {}", e);
    }

    let cfg = read_stats_retention_config(app);
    let _ = stats_store.prune(cfg);

    if any_stats_written {
        let _ = app.emit(events::EVENT_STATS_CHANGED, ());
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
