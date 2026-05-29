use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::AppHandle;
use tauri::Emitter;

use crate::commands::{CommandError, CommandResult};
use crate::events;

#[cfg(desktop)]
use tauri_plugin_store::StoreExt;

const POLICY_STATE_KEY: &str = "policy_state";
const POLICY_EFFECTIVE_VALUES_KEY: &str = "policy_effective_values";
const POLICY_CLOUD_CANDIDATE_KEY: &str = "policy_cloud_candidate";
const LICENSE_STATE_KEY: &str = "license_state";
const DISABLE_PRODUCT_ANALYTICS_POLICY_PATH: &str = "disable_product_analytics";
const POSTHOG_ANALYTICS_ENABLED_KEY: &str = "posthog_analytics_enabled";

fn policy_setting_targets(path: &str, value: &Value) -> Vec<(String, Value)> {
    match (path, value) {
        // Keep the policy contract product-oriented while translating it onto the
        // concrete desktop setting that the current analytics transport reads.
        (DISABLE_PRODUCT_ANALYTICS_POLICY_PATH, Value::Bool(true)) => vec![(
            POSTHOG_ANALYTICS_ENABLED_KEY.to_string(),
            Value::Bool(false),
        )],
        _ => vec![(path.to_string(), value.clone())],
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicySyncRequest {
    #[serde(default)]
    pub policy_pack: Option<Value>,
}

#[cfg(desktop)]
fn load_policy_state(app: &AppHandle) -> CommandResult<crate::policy::PolicyState> {
    let store = app
        .store("settings.json")
        .map_err(|e| CommandError::unknown(format!("Failed to open settings store: {e}")))?;
    let raw = store.get(POLICY_STATE_KEY);
    Ok(crate::policy::policy_state_for_command(raw, Utc::now()))
}

#[cfg(desktop)]
fn is_policy_eligible(app: &AppHandle) -> CommandResult<bool> {
    let store = app
        .store("settings.json")
        .map_err(|e| CommandError::unknown(format!("Failed to open settings store: {e}")))?;

    let raw = store.get(LICENSE_STATE_KEY);
    let Some(Value::Object(map)) = raw else {
        return Ok(false);
    };

    let status = map
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("signed_out");
    let has_org = map.get("org").and_then(|v| v.as_object()).is_some();
    Ok(has_org && matches!(status, "active" | "grace"))
}

#[cfg(desktop)]
fn persist_policy(
    app: &AppHandle,
    state: &crate::policy::PolicyState,
    effective_values: &serde_json::Map<String, Value>,
) -> CommandResult<()> {
    let store = app
        .store("settings.json")
        .map_err(|e| CommandError::unknown(format!("Failed to open settings store: {e}")))?;

    store.set(
        POLICY_STATE_KEY.to_string(),
        serde_json::to_value(state)
            .map_err(|e| CommandError::unknown(format!("Failed to serialize policy state: {e}")))?,
    );
    store.set(
        POLICY_EFFECTIVE_VALUES_KEY.to_string(),
        Value::Object(effective_values.clone()),
    );

    for (path, value) in effective_values {
        for (target_path, target_value) in policy_setting_targets(path, value) {
            store.set(target_path, target_value);
        }
    }

    store
        .save()
        .map_err(|e| CommandError::unknown(format!("Failed to save settings store: {e}")))?;

    let mut payload = crate::SettingsChangedPayload::new();
    payload.insert("policy_state_changed".to_string(), json!(true));
    payload.insert("policy_constraints_applied".to_string(), json!(true));
    payload.insert(
        "policy_enforced_count".to_string(),
        json!(state.enforced_count),
    );
    let _ = app.emit(events::EVENT_SETTINGS_CHANGED, payload);
    let _ = app.emit(events::EVENT_POLICY_STATE_CHANGED, state.clone());

    Ok(())
}

#[cfg(desktop)]
pub(crate) fn clear_cached_policy_state(app: &AppHandle) -> CommandResult<()> {
    let store = app
        .store("settings.json")
        .map_err(|e| CommandError::unknown(format!("Failed to open settings store: {e}")))?;

    let _ = store.delete(POLICY_STATE_KEY);
    let _ = store.delete(POLICY_EFFECTIVE_VALUES_KEY);
    let _ = store.delete(POLICY_CLOUD_CANDIDATE_KEY);

    store
        .save()
        .map_err(|e| CommandError::unknown(format!("Failed to save settings store: {e}")))?;

    let cleared = crate::policy::PolicyState::default();
    let mut payload = crate::SettingsChangedPayload::new();
    payload.insert("policy_state_changed".to_string(), json!(true));
    payload.insert("policy_constraints_applied".to_string(), json!(false));
    payload.insert("policy_enforced_count".to_string(), json!(0));
    let _ = app.emit(events::EVENT_SETTINGS_CHANGED, payload);
    let _ = app.emit(events::EVENT_POLICY_STATE_CHANGED, cleared);

    Ok(())
}

#[cfg(not(desktop))]
pub(crate) fn clear_cached_policy_state(_app: &AppHandle) -> CommandResult<()> {
    Ok(())
}

#[cfg(desktop)]
#[tauri::command]
pub async fn policy_get_state(app: AppHandle) -> CommandResult<crate::policy::PolicyState> {
    load_policy_state(&app)
}

#[cfg(not(desktop))]
#[tauri::command]
pub async fn policy_get_state(_app: AppHandle) -> CommandResult<crate::policy::PolicyState> {
    Ok(crate::policy::PolicyState::default())
}

#[cfg(desktop)]
#[tauri::command]
pub async fn policy_sync(
    app: AppHandle,
    request: Option<PolicySyncRequest>,
) -> CommandResult<crate::policy::PolicyState> {
    let now = Utc::now();
    let eligible = is_policy_eligible(&app)?;
    let current = load_policy_state(&app)?;

    if !eligible {
        let next = crate::policy::PolicyState {
            eligible: false,
            source: crate::policy::PolicySource::None,
            is_valid: true,
            last_sync_at: Some(now),
            last_success_at: current.last_success_at,
            failure_reason: Some("ineligible_org_membership".to_string()),
            ..crate::policy::PolicyState::default()
        };
        persist_policy(&app, &next, &serde_json::Map::new())?;
        return Ok(next);
    }

    let store = app
        .store("settings.json")
        .map_err(|e| CommandError::unknown(format!("Failed to open settings store: {e}")))?;

    let candidate = request
        .and_then(|req| req.policy_pack)
        .or_else(|| store.get(POLICY_CLOUD_CANDIDATE_KEY));

    let Some(candidate) = candidate else {
        let next = crate::policy::policy_state_for_sync_failure(&current, now, "no_policy_payload");
        persist_policy(&app, &next, &serde_json::Map::new())?;
        return Ok(next);
    };

    let outcome = match crate::policy::validate_cloud_policy_candidate(&candidate, &current, now) {
        Ok(outcome) => outcome,
        Err(msg) => {
            let next = crate::policy::policy_state_for_sync_failure(
                &current,
                now,
                &format!("policy_invalid:{msg}"),
            );
            persist_policy(&app, &next, &serde_json::Map::new())?;
            return Ok(next);
        }
    };

    persist_policy(&app, &outcome.policy_state, &outcome.effective_values)?;
    Ok(outcome.policy_state)
}

#[cfg(not(desktop))]
#[tauri::command]
pub async fn policy_sync(
    _app: AppHandle,
    _request: Option<PolicySyncRequest>,
) -> CommandResult<crate::policy::PolicyState> {
    Ok(crate::policy::PolicyState::default())
}

#[cfg(desktop)]
#[tauri::command]
pub async fn policy_export_diagnostics(
    app: AppHandle,
) -> CommandResult<crate::policy::PolicyDiagnosticExport> {
    let state = load_policy_state(&app)?;
    let store = app
        .store("settings.json")
        .map_err(|e| CommandError::unknown(format!("Failed to open settings store: {e}")))?;
    let effective_values = store
        .get(POLICY_EFFECTIVE_VALUES_KEY)
        .and_then(|v| v.as_object().cloned());

    Ok(crate::policy::build_policy_diagnostic_export(
        state,
        Utc::now(),
        effective_values.as_ref(),
    ))
}

#[cfg(not(desktop))]
#[tauri::command]
pub async fn policy_export_diagnostics(
    _app: AppHandle,
) -> CommandResult<crate::policy::PolicyDiagnosticExport> {
    Ok(crate::policy::build_policy_diagnostic_export(
        crate::policy::PolicyState::default(),
        Utc::now(),
        None,
    ))
}

#[cfg(test)]
mod tests {
    use super::policy_setting_targets;
    use serde_json::{json, Value};

    #[test]
    fn maps_disable_product_analytics_to_posthog_setting() {
        let targets = policy_setting_targets("disable_product_analytics", &Value::Bool(true));

        assert_eq!(
            targets,
            vec![("posthog_analytics_enabled".to_string(), Value::Bool(false))]
        );
    }

    #[test]
    fn passes_through_direct_setting_paths() {
        let targets = policy_setting_targets("request_logs_privacy_mode", &json!(true));

        assert_eq!(
            targets,
            vec![("request_logs_privacy_mode".to_string(), Value::Bool(true))]
        );
    }
}
