use chrono::{DateTime, Duration as ChronoDuration, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use tauri::State;

use crate::commands::CommandResult;
use crate::cost::openai::UsdMicros;
use crate::stats::{CostEvent, CostKind, StatsStore};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CostSummaryResponse {
    pub timeframe: String,
    pub total_usd_micros: UsdMicros,
    pub events_total: u64,
    pub events_with_cost: u64,
    pub earliest_included_at: Option<DateTime<Utc>>,
    pub latest_included_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProviderCostTotal {
    pub provider: String,
    pub total_usd_micros: UsdMicros,
    pub events_total: u64,
    pub events_with_cost: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CostByProviderResponse {
    pub timeframe: String,
    pub providers: Vec<ProviderCostTotal>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetCostSummaryParams {
    pub timeframe: String,
    pub kind: Option<String>,
    #[serde(alias = "sttModelKeys")]
    pub stt_model_keys: Option<Vec<String>>,
    #[serde(alias = "llmModelKeys")]
    pub llm_model_keys: Option<Vec<String>>,
    #[serde(alias = "excludeFreeTier")]
    pub exclude_free_tier: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct StatsCacheKey {
    kind: Option<String>,
    timeframe: String,
    stt_model_keys: Vec<String>,
    llm_model_keys: Vec<String>,
    exclude_free_tier: bool,
}

fn make_cache_key_for_summary(
    timeframe: &str,
    kind: &Option<String>,
    stt_model_keys: &Option<Vec<String>>,
    llm_model_keys: &Option<Vec<String>>,
    exclude_free_tier: bool,
) -> String {
    let mut stt = stt_model_keys.clone().unwrap_or_default();
    let mut llm = llm_model_keys.clone().unwrap_or_default();
    stt.sort();
    llm.sort();

    let key = StatsCacheKey {
        kind: kind
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        timeframe: timeframe.trim().to_string(),
        stt_model_keys: stt,
        llm_model_keys: llm,
        exclude_free_tier,
    };

    serde_json::to_string(&key).unwrap_or_else(|_| format!("{}:{}", timeframe, exclude_free_tier))
}

fn make_cache_key_for_by_provider(params: &GetCostSummaryParams) -> String {
    make_cache_key_for_summary(
        params.timeframe.as_str(),
        &params.kind,
        &params.stt_model_keys,
        &params.llm_model_keys,
        params.exclude_free_tier.unwrap_or(false),
    )
}

fn cutoff_for_timeframe(timeframe: &str) -> Option<DateTime<Utc>> {
    let now = Utc::now();
    match timeframe {
        "24h" => Some(now - ChronoDuration::hours(24)),
        "7d" => Some(now - ChronoDuration::days(7)),
        "30d" => Some(now - ChronoDuration::days(30)),
        "90d" => Some(now - ChronoDuration::days(90)),
        "all" => None,
        _ => Some(now - ChronoDuration::days(30)),
    }
}

fn is_stats_file_name(name: &str) -> bool {
    name.starts_with("cost-events-") && name.ends_with(".jsonl")
}

fn parse_kind_filter(kind: Option<String>) -> Option<CostKind> {
    let kind = kind?.trim().to_lowercase();
    match kind.as_str() {
        "stt" => Some(CostKind::Stt),
        "llm" => Some(CostKind::Llm),
        "all" => None,
        _ => None,
    }
}

/// Returns a single aggregated number: total spend across all providers/models/kinds.
///
/// NOTE: This uses `estimated_cost_usd_micros` when present. Events without a cost estimate
/// are counted in `events_total` but excluded from the total.
#[tauri::command]
pub fn get_cost_summary(
    stats_store: State<'_, StatsStore>,
    timeframe: String,
    kind: Option<String>,
    stt_model_keys: Option<Vec<String>>,
    llm_model_keys: Option<Vec<String>>,
    exclude_free_tier: Option<bool>,
) -> CommandResult<CostSummaryResponse> {
    let exclude_free_tier = exclude_free_tier.unwrap_or(false);
    let cache_key = make_cache_key_for_summary(
        timeframe.as_str(),
        &kind,
        &stt_model_keys,
        &llm_model_keys,
        exclude_free_tier,
    );

    if let Some(cached) = stats_store.cache_get_cost_summary::<CostSummaryResponse>(&cache_key) {
        return Ok(cached);
    }

    let timeframe = timeframe.trim().to_string();
    let cutoff = cutoff_for_timeframe(&timeframe);
    let kind_filter = parse_kind_filter(kind);
    let selected_stt_model_keys: Option<std::collections::HashSet<String>> = stt_model_keys
        .filter(|v| !v.is_empty())
        .map(|v| v.into_iter().collect());
    let selected_llm_model_keys: Option<std::collections::HashSet<String>> = llm_model_keys
        .filter(|v| !v.is_empty())
        .map(|v| v.into_iter().collect());

    let mut total_usd_micros: u128 = 0;
    let mut events_total: u64 = 0;
    let mut events_with_cost: u64 = 0;
    let mut earliest_included_at: Option<DateTime<Utc>> = None;
    let mut latest_included_at: Option<DateTime<Utc>> = None;

    let dir = stats_store.dir().to_path_buf();
    if !dir.exists() {
        return Ok(CostSummaryResponse {
            timeframe,
            total_usd_micros: 0,
            events_total: 0,
            events_with_cost: 0,
            earliest_included_at: None,
            latest_included_at: None,
        });
    }

    let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !is_stats_file_name(&name) {
            continue;
        }

        let file = fs::File::open(&path).map_err(|e| e.to_string())?;
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

            if let Some(cut) = cutoff {
                if ev.created_at < cut {
                    continue;
                }
            }

            if let Some(kind_filter) = kind_filter {
                if ev.kind != kind_filter {
                    continue;
                }
            }

            if exclude_free_tier && ev.is_free_tier {
                continue;
            }

            // Per-kind model filters.
            // - If kind is explicitly filtered, only the corresponding model filter applies.
            // - If kind is "all", apply STT filters to STT events and LLM filters to LLM events.
            let keys_to_apply = match ev.kind {
                CostKind::Stt => selected_stt_model_keys.as_ref(),
                CostKind::Llm => selected_llm_model_keys.as_ref(),
            };

            if let Some(keys) = keys_to_apply {
                // Model filters are based on `${provider}::${model}` keys.
                let Some(model) = ev.model.as_deref() else {
                    continue;
                };
                let key = format!("{}::{}", ev.provider, model);
                if !keys.contains(&key) {
                    continue;
                }
            }

            events_total = events_total.saturating_add(1);
            earliest_included_at = match earliest_included_at {
                None => Some(ev.created_at),
                Some(t) => Some(std::cmp::min(t, ev.created_at)),
            };
            latest_included_at = match latest_included_at {
                None => Some(ev.created_at),
                Some(t) => Some(std::cmp::max(t, ev.created_at)),
            };

            if let Some(micros) = ev.estimated_cost_usd_micros {
                events_with_cost = events_with_cost.saturating_add(1);
                total_usd_micros = total_usd_micros.saturating_add(micros as u128);
            }
        }
    }

    let out = CostSummaryResponse {
        timeframe,
        total_usd_micros: (total_usd_micros.min(u128::from(u64::MAX))) as u64,
        events_total,
        events_with_cost,
        earliest_included_at,
        latest_included_at,
    };

    stats_store.cache_put_cost_summary(cache_key, &out);
    Ok(out)
}

/// Same as `get_cost_summary`, but takes a single params object and supports both
/// snake_case and camelCase argument names via serde aliases.
#[tauri::command]
pub fn get_cost_summary_v2(
    stats_store: State<'_, StatsStore>,
    params: GetCostSummaryParams,
) -> CommandResult<CostSummaryResponse> {
    get_cost_summary(
        stats_store,
        params.timeframe,
        params.kind,
        params.stt_model_keys,
        params.llm_model_keys,
        params.exclude_free_tier,
    )
}

/// Returns spend totals grouped by provider for a timeframe (and the same filters
/// supported by `get_cost_summary_v2`).
///
/// This is intended for UI breakdowns (e.g. a per-provider totals list under the
/// Total spend card).
#[tauri::command]
pub fn get_cost_by_provider_v2(
    stats_store: State<'_, StatsStore>,
    params: GetCostSummaryParams,
) -> CommandResult<CostByProviderResponse> {
    let cache_key = make_cache_key_for_by_provider(&params);
    if let Some(cached) =
        stats_store.cache_get_cost_by_provider::<CostByProviderResponse>(&cache_key)
    {
        return Ok(cached);
    }

    let timeframe = params.timeframe.trim().to_string();
    let cutoff = cutoff_for_timeframe(&timeframe);
    let kind_filter = parse_kind_filter(params.kind);
    let selected_stt_model_keys: Option<std::collections::HashSet<String>> = params
        .stt_model_keys
        .filter(|v| !v.is_empty())
        .map(|v| v.into_iter().collect());
    let selected_llm_model_keys: Option<std::collections::HashSet<String>> = params
        .llm_model_keys
        .filter(|v| !v.is_empty())
        .map(|v| v.into_iter().collect());
    let exclude_free_tier = params.exclude_free_tier.unwrap_or(false);

    // provider -> (total_usd_micros, events_total, events_with_cost)
    let mut by_provider: HashMap<String, (u128, u64, u64)> = HashMap::new();

    let dir = stats_store.dir().to_path_buf();
    if !dir.exists() {
        return Ok(CostByProviderResponse {
            timeframe,
            providers: Vec::new(),
        });
    }

    let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !is_stats_file_name(&name) {
            continue;
        }

        let file = fs::File::open(&path).map_err(|e| e.to_string())?;
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

            if let Some(cut) = cutoff {
                if ev.created_at < cut {
                    continue;
                }
            }

            if let Some(kind_filter) = kind_filter {
                if ev.kind != kind_filter {
                    continue;
                }
            }

            if exclude_free_tier && ev.is_free_tier {
                continue;
            }

            // Per-kind model filters.
            let keys_to_apply = match ev.kind {
                CostKind::Stt => selected_stt_model_keys.as_ref(),
                CostKind::Llm => selected_llm_model_keys.as_ref(),
            };

            if let Some(keys) = keys_to_apply {
                let Some(model) = ev.model.as_deref() else {
                    continue;
                };
                let key = format!("{}::{}", ev.provider, model);
                if !keys.contains(&key) {
                    continue;
                }
            }

            let entry = by_provider
                .entry(ev.provider.clone())
                .or_insert((0u128, 0u64, 0u64));
            entry.1 = entry.1.saturating_add(1);

            if let Some(micros) = ev.estimated_cost_usd_micros {
                entry.2 = entry.2.saturating_add(1);
                entry.0 = entry.0.saturating_add(micros as u128);
            }
        }
    }

    let mut providers: Vec<ProviderCostTotal> = by_provider
        .into_iter()
        .map(
            |(provider, (total, events_total, events_with_cost))| ProviderCostTotal {
                provider,
                total_usd_micros: (total.min(u128::from(u64::MAX))) as u64,
                events_total,
                events_with_cost,
            },
        )
        .collect();

    // Sort by spend desc, then provider name asc.
    providers.sort_by(|a, b| {
        b.total_usd_micros
            .cmp(&a.total_usd_micros)
            .then_with(|| a.provider.cmp(&b.provider))
    });

    let out = CostByProviderResponse {
        timeframe,
        providers,
    };
    stats_store.cache_put_cost_by_provider(cache_key, &out);
    Ok(out)
}
