//! Typed, feature-shaped settings reads.
//!
//! `settings/store.rs` owns store loading and low-level coercion. This Module owns the higher-level
//! views that backend features actually consume so runtime code stops repeating the same raw key
//! list and normalization rules.

use chrono::Duration as ChronoDuration;
#[cfg(desktop)]
use serde::de::DeserializeOwned;

use crate::commands::text::OutputMode;
use crate::request_log::{RequestLogsRetentionConfig, RequestLogsRetentionMode};
use crate::sessions::quick_action_lifecycle::QuickAskGlobalConfig;
use crate::settings::default_values;
use crate::settings::store::SettingsReadMode;
use crate::stats::StatsRetentionConfig;

#[cfg(desktop)]
use crate::settings::store::{get_settings_store, store_get_u64_clamped, SettingsStore};
#[cfg(desktop)]
use tauri::AppHandle;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OutputSettingsView {
    pub(crate) mode: OutputMode,
    pub(crate) hit_enter: bool,
    pub(crate) clipboard_privacy_mode: bool,
    pub(crate) smart_paste_protection: bool,
}

fn sanitize_output_settings(mode: OutputMode, hit_enter: bool) -> OutputSettingsView {
    let hit_enter = if matches!(mode, OutputMode::Clipboard) {
        false
    } else {
        hit_enter
    };

    OutputSettingsView {
        mode,
        hit_enter,
        clipboard_privacy_mode: default_values::DEFAULT_OUTPUT_CLIPBOARD_PRIVACY_MODE,
        smart_paste_protection: default_values::DEFAULT_OUTPUT_SMART_PASTE_PROTECTION,
    }
}

fn build_request_logs_retention(mode: &str, amount: u64, days: u64) -> RequestLogsRetentionConfig {
    let mode = if mode == "time" {
        RequestLogsRetentionMode::Time
    } else {
        RequestLogsRetentionMode::Amount
    };

    RequestLogsRetentionConfig {
        mode,
        amount: amount.clamp(1, 200) as usize,
        time_retention: if days == 0 {
            None
        } else {
            Some(ChronoDuration::days(days as i64))
        },
    }
}

fn build_stats_retention(unit: &str, value: f64, max_bytes: u64) -> StatsRetentionConfig {
    let value = if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    };

    let time_retention = if value == 0.0 {
        None
    } else if unit == "hours" {
        Some(ChronoDuration::milliseconds((value * 3_600_000.0) as i64))
    } else {
        Some(ChronoDuration::milliseconds(
            (value * 24.0 * 3_600_000.0) as i64,
        ))
    };

    StatsRetentionConfig {
        time_retention,
        max_bytes,
    }
}

fn provider_free_tier_key(provider: &str) -> Option<(&'static str, bool)> {
    match provider {
        // Default to true to match the existing UI/runtime expectation for these providers.
        "cerebras" => Some(("cerebras_free_tier", true)),
        "groq" => Some(("groq_free_tier", true)),
        "cohere" => Some(("cohere_free_tier", true)),
        "assemblyai" => Some(("assemblyai_free_tier", true)),
        "speechmatics" => Some(("speechmatics_free_tier", true)),
        _ => None,
    }
}

#[cfg(desktop)]
fn store_get_deserialized<T: DeserializeOwned>(store: &SettingsStore, key: &str) -> Option<T> {
    store
        .get(key)
        .and_then(|value| serde_json::from_value(value).ok())
}

#[cfg(desktop)]
fn read_setting<T: DeserializeOwned + Clone>(
    app: &AppHandle,
    mode: SettingsReadMode,
    key: &str,
    default: T,
) -> T {
    let Some(store) = get_settings_store(app, mode) else {
        return default;
    };

    store_get_deserialized(&store, key).unwrap_or(default)
}

#[cfg(desktop)]
pub(crate) fn read_output_settings_view(
    app: &AppHandle,
    mode: SettingsReadMode,
) -> OutputSettingsView {
    let output_mode = read_setting(
        app,
        mode,
        "output_mode",
        default_values::DEFAULT_OUTPUT_MODE.to_string(),
    );
    let mut view = sanitize_output_settings(
        OutputMode::from_str(&output_mode),
        read_setting(
            app,
            mode,
            "output_hit_enter",
            default_values::DEFAULT_OUTPUT_HIT_ENTER,
        ),
    );
    view.clipboard_privacy_mode = read_setting(
        app,
        mode,
        "output_clipboard_privacy_mode",
        default_values::DEFAULT_OUTPUT_CLIPBOARD_PRIVACY_MODE,
    );
    view.smart_paste_protection = read_setting(
        app,
        mode,
        "output_smart_paste_protection",
        default_values::DEFAULT_OUTPUT_SMART_PASTE_PROTECTION,
    );
    view
}

#[cfg(not(desktop))]
pub(crate) fn read_output_settings_view(
    _app: &tauri::AppHandle,
    _mode: SettingsReadMode,
) -> OutputSettingsView {
    sanitize_output_settings(OutputMode::Paste, default_values::DEFAULT_OUTPUT_HIT_ENTER)
}

#[cfg(desktop)]
pub(crate) fn read_quick_ask_global_config(
    app: &AppHandle,
    mode: SettingsReadMode,
) -> QuickAskGlobalConfig {
    QuickAskGlobalConfig {
        provider: read_setting(app, mode, "quick_ask_provider", None::<String>),
        model: read_setting(app, mode, "quick_ask_model", None::<String>),
        system_prompt: read_setting(app, mode, "quick_ask_system_prompt", None::<String>),
        openai_reasoning_effort: read_setting(
            app,
            mode,
            "quick_ask_openai_reasoning_effort",
            None::<String>,
        ),
        gemini_thinking_budget: read_setting(
            app,
            mode,
            "quick_ask_gemini_thinking_budget",
            None::<i64>,
        ),
        gemini_thinking_level: read_setting(
            app,
            mode,
            "quick_ask_gemini_thinking_level",
            None::<String>,
        ),
        anthropic_thinking_budget: read_setting(
            app,
            mode,
            "quick_ask_anthropic_thinking_budget",
            None::<i64>,
        ),
        fallback_provider: read_setting(app, mode, "llm_provider", None::<String>),
        conversation_history_enabled: read_setting(
            app,
            mode,
            "quick_ask_conversation_history_enabled",
            default_values::DEFAULT_QUICK_ASK_CONVERSATION_HISTORY_ENABLED,
        ),
        conversation_history_count_raw: read_setting(
            app,
            mode,
            "quick_ask_conversation_history_count",
            default_values::DEFAULT_QUICK_ASK_CONVERSATION_HISTORY_COUNT as u64,
        ),
        request_logs_privacy_mode: read_setting(
            app,
            mode,
            "request_logs_privacy_mode",
            default_values::DEFAULT_REQUEST_LOGS_PRIVACY_MODE,
        ),
    }
}

#[cfg(not(desktop))]
pub(crate) fn read_quick_ask_global_config(
    _app: &tauri::AppHandle,
    _mode: SettingsReadMode,
) -> QuickAskGlobalConfig {
    QuickAskGlobalConfig {
        conversation_history_enabled:
            default_values::DEFAULT_QUICK_ASK_CONVERSATION_HISTORY_ENABLED,
        conversation_history_count_raw: default_values::DEFAULT_QUICK_ASK_CONVERSATION_HISTORY_COUNT
            as u64,
        request_logs_privacy_mode: default_values::DEFAULT_REQUEST_LOGS_PRIVACY_MODE,
        ..Default::default()
    }
}

#[cfg(desktop)]
pub(crate) fn read_request_logs_retention(
    app: &AppHandle,
    mode: SettingsReadMode,
) -> RequestLogsRetentionConfig {
    let Some(store) = get_settings_store(app, mode) else {
        return build_request_logs_retention(
            default_values::DEFAULT_REQUEST_LOGS_RETENTION_MODE,
            default_values::DEFAULT_REQUEST_LOGS_RETENTION_AMOUNT as u64,
            default_values::DEFAULT_REQUEST_LOGS_RETENTION_DAYS as u64,
        );
    };

    build_request_logs_retention(
        &read_setting(
            app,
            mode,
            "request_logs_retention_mode",
            default_values::DEFAULT_REQUEST_LOGS_RETENTION_MODE.to_string(),
        ),
        store_get_u64_clamped(
            &store,
            "request_logs_retention_amount",
            default_values::DEFAULT_REQUEST_LOGS_RETENTION_AMOUNT as u64,
            1,
            200,
        ),
        store_get_u64_clamped(
            &store,
            "request_logs_retention_days",
            default_values::DEFAULT_REQUEST_LOGS_RETENTION_DAYS as u64,
            0,
            36_500,
        ),
    )
}

#[cfg(not(desktop))]
pub(crate) fn read_request_logs_retention(
    _app: &tauri::AppHandle,
    _mode: SettingsReadMode,
) -> RequestLogsRetentionConfig {
    build_request_logs_retention(
        default_values::DEFAULT_REQUEST_LOGS_RETENTION_MODE,
        default_values::DEFAULT_REQUEST_LOGS_RETENTION_AMOUNT as u64,
        default_values::DEFAULT_REQUEST_LOGS_RETENTION_DAYS as u64,
    )
}

#[cfg(desktop)]
pub(crate) fn read_stats_retention_config(
    app: &AppHandle,
    mode: SettingsReadMode,
) -> StatsRetentionConfig {
    build_stats_retention(
        &read_setting(
            app,
            mode,
            "stats_retention_unit",
            default_values::DEFAULT_STATS_RETENTION_UNIT.to_string(),
        ),
        read_setting(
            app,
            mode,
            "stats_retention_value",
            default_values::DEFAULT_STATS_RETENTION_VALUE,
        ),
        read_setting(
            app,
            mode,
            "stats_retention_max_bytes",
            default_values::DEFAULT_STATS_RETENTION_MAX_BYTES,
        ),
    )
}

#[cfg(not(desktop))]
pub(crate) fn read_stats_retention_config(
    _app: &tauri::AppHandle,
    _mode: SettingsReadMode,
) -> StatsRetentionConfig {
    build_stats_retention(
        default_values::DEFAULT_STATS_RETENTION_UNIT,
        default_values::DEFAULT_STATS_RETENTION_VALUE,
        default_values::DEFAULT_STATS_RETENTION_MAX_BYTES,
    )
}

#[cfg(desktop)]
pub(crate) fn is_provider_free_tier_enabled(
    app: &AppHandle,
    provider: &str,
    mode: SettingsReadMode,
) -> bool {
    let Some((key, default)) = provider_free_tier_key(provider) else {
        return false;
    };
    read_setting(app, mode, key, default)
}

#[cfg(not(desktop))]
pub(crate) fn is_provider_free_tier_enabled(
    _app: &tauri::AppHandle,
    _provider: &str,
    _mode: SettingsReadMode,
) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_settings_force_hit_enter_off_for_clipboard_mode() {
        let view = sanitize_output_settings(OutputMode::Clipboard, true);
        assert_eq!(view.mode, OutputMode::Clipboard);
        assert!(!view.hit_enter);
    }

    #[test]
    fn request_log_retention_clamps_amount_and_supports_time_mode() {
        let amount_cfg = build_request_logs_retention("amount", 0, 7);
        assert_eq!(amount_cfg.mode, RequestLogsRetentionMode::Amount);
        assert_eq!(amount_cfg.amount, 1);
        assert_eq!(amount_cfg.time_retention, Some(ChronoDuration::days(7)));

        let time_cfg = build_request_logs_retention("time", 500, 0);
        assert_eq!(time_cfg.mode, RequestLogsRetentionMode::Time);
        assert_eq!(time_cfg.amount, 200);
        assert_eq!(time_cfg.time_retention, None);
    }

    #[test]
    fn stats_retention_supports_hours_days_and_disabled() {
        assert_eq!(
            build_stats_retention("hours", 2.5, 123).time_retention,
            Some(ChronoDuration::milliseconds(9_000_000))
        );
        assert_eq!(
            build_stats_retention("days", 1.0, 123).time_retention,
            Some(ChronoDuration::milliseconds(86_400_000))
        );
        assert_eq!(build_stats_retention("days", 0.0, 123).time_retention, None);
    }

    #[test]
    fn free_tier_key_defaults_match_supported_providers() {
        assert_eq!(
            provider_free_tier_key("groq"),
            Some(("groq_free_tier", true))
        );
        assert_eq!(
            provider_free_tier_key("cohere"),
            Some(("cohere_free_tier", true))
        );
        assert_eq!(provider_free_tier_key("openai"), None);
    }
}
