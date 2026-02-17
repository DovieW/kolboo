//! Managed inference scaffolding.
//!
//! This module is intentionally minimal for Phase 1 setup.
//! Subsequent phases will add auth preflight, quota checks,
//! deterministic error mapping, and idempotent metering.

pub mod errors;

pub const CMD_STT_TRANSCRIBE: &str = "managed_inference_stt_transcribe";
pub const CMD_LLM_COMPLETE: &str = "managed_inference_llm_complete";
pub const CMD_USAGE_STATE: &str = "managed_inference_get_usage_state";

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManagedInferenceMode {
    Managed,
    Byok,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagedErrorCategory {
    Unauthorized,
    Ineligible,
    OverQuota,
    TemporarilyUnavailable,
}

#[derive(Debug, Clone)]
pub struct ManagedError {
    pub category: ManagedErrorCategory,
    pub code: String,
    pub message: String,
    pub request_id: Option<String>,
    pub retry_after_seconds: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct ManagedUsageCounter {
    pub metric: String,
    pub used: u64,
    pub limit: u64,
    pub warning_thresholds: Vec<u64>,
    pub window: String,
}

#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct ManagedUsageState {
    pub tier: crate::licensing::LicenseTier,
    pub mode: ManagedInferenceMode,
    pub counters: Vec<ManagedUsageCounter>,
}

fn mode_for_license(state: &crate::licensing::LicenseState) -> ManagedInferenceMode {
    match (state.tier, state.status) {
        (
            crate::licensing::LicenseTier::Personal,
            crate::licensing::LicenseStatus::Active | crate::licensing::LicenseStatus::Grace,
        ) => ManagedInferenceMode::Managed,
        _ => ManagedInferenceMode::Byok,
    }
}

#[cfg(desktop)]
#[tauri::command]
pub fn managed_inference_get_usage_state(
    app: tauri::AppHandle,
) -> Result<ManagedUsageState, String> {
    use chrono::Utc;
    use tauri_plugin_store::StoreExt;

    let store = app
        .store("settings.json")
        .map_err(|e| format!("Failed to open settings store: {e}"))?;
    let license_state =
        crate::licensing::normalize_license_state(store.get("license_state"), Utc::now());
    let mode = mode_for_license(&license_state);

    Ok(ManagedUsageState {
        tier: license_state.tier,
        mode,
        counters: vec![
            ManagedUsageCounter {
                metric: "stt_seconds".to_string(),
                used: license_state.usage.stt_seconds_used,
                limit: license_state.limits.stt_seconds_monthly,
                warning_thresholds: vec![50, 80, 95],
                window: "monthly".to_string(),
            },
            ManagedUsageCounter {
                metric: "llm_tokens".to_string(),
                used: license_state.usage.llm_tokens_used,
                limit: license_state.limits.llm_tokens_monthly,
                warning_thresholds: vec![50, 80, 95],
                window: "monthly".to_string(),
            },
            ManagedUsageCounter {
                metric: "managed_requests".to_string(),
                used: license_state.usage.requests_today,
                limit: license_state.limits.requests_per_day,
                warning_thresholds: vec![50, 80, 95],
                window: "daily".to_string(),
            },
        ],
    })
}

#[cfg(not(desktop))]
#[tauri::command]
pub fn managed_inference_get_usage_state(
    _app: tauri::AppHandle,
) -> Result<ManagedUsageState, String> {
    Ok(ManagedUsageState {
        tier: crate::licensing::LicenseTier::Community,
        mode: ManagedInferenceMode::Byok,
        counters: vec![],
    })
}
