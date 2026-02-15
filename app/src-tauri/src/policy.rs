use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const SUPPORTED_POLICY_PATHS: &[&str] = &[
    "rewrite_llm_enabled",
    "stt_provider",
    "stt_model",
    "stt_language",
    "stt_timeout_seconds",
    "llm_provider",
    "llm_model",
    "overlay_mode",
    "widget_position",
    "output_mode",
    "output_hit_enter",
    "request_logs_privacy_mode",
    "quick_ask_provider",
    "quick_ask_model",
    "quick_ask_system_prompt",
    "quick_ask_dismiss_mode",
    "quick_ask_include_selected_text",
    "quick_replace_enabled",
    "quick_replace_provider",
    "quick_replace_model",
    "quick_replace_system_prompt",
    "quick_ask_openai_reasoning_effort",
    "quick_ask_gemini_thinking_budget",
    "quick_ask_gemini_thinking_level",
    "quick_ask_anthropic_thinking_budget",
    "openai_reasoning_effort",
    "gemini_thinking_budget",
    "gemini_thinking_level",
    "anthropic_thinking_budget",
    "rewrite_active_window_ocr_mode",
    "quick_replace_active_window_ocr_mode",
    "quick_ask_active_window_ocr_mode",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PolicySource {
    #[default]
    None,
    File,
    Cloud,
    Cached,
    DegradedExpired,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PolicyEnforcedField {
    pub path: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PolicyState {
    pub source: PolicySource,
    #[serde(default)]
    pub eligible: bool,
    pub is_valid: bool,
    pub active_policy_id: Option<String>,
    pub active_version: Option<u64>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_updated: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub failure_reason: Option<String>,
    pub enforced_count: u64,
    pub version: Option<String>,
    #[serde(default)]
    pub enforced_fields: Vec<PolicyEnforcedField>,
}

impl Default for PolicyState {
    fn default() -> Self {
        Self {
            source: PolicySource::None,
            eligible: false,
            is_valid: true,
            active_policy_id: None,
            active_version: None,
            last_sync_at: None,
            last_success_at: None,
            last_updated: None,
            expires_at: None,
            failure_reason: None,
            enforced_count: 0,
            version: None,
            enforced_fields: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PolicyDiagnosticField {
    pub path: String,
    #[serde(default)]
    pub effective_value: Option<Value>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PolicyDiagnosticExport {
    pub generated_at: DateTime<Utc>,
    pub policy_state: PolicyState,
    pub enforced_fields: Vec<PolicyDiagnosticField>,
    pub redaction_applied: bool,
}

fn parse_datetime(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let raw = value.and_then(|x| x.as_str())?;
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_policy_source(value: Option<&Value>) -> PolicySource {
    match value.and_then(|x| x.as_str()) {
        Some("file") => PolicySource::File,
        Some("cloud") => PolicySource::Cloud,
        Some("cached") => PolicySource::Cached,
        Some("degraded_expired") => PolicySource::DegradedExpired,
        _ => PolicySource::None,
    }
}

fn parse_version_num(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(v) = trimmed.parse::<u64>() {
        return Some(v);
    }

    let without_v = trimmed.strip_prefix('v').unwrap_or(trimmed);
    without_v.parse::<u64>().ok()
}

fn parse_policy_version(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(v)) => {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Some(Value::Number(v)) => v.as_u64().map(|n| n.to_string()),
        _ => None,
    }
}

fn parse_constraints(
    value: Option<&Value>,
) -> Result<(Vec<PolicyEnforcedField>, serde_json::Map<String, Value>), String> {
    let Some(Value::Object(map)) = value else {
        return Err("Policy is missing a constraints object".to_string());
    };

    let mut enforced_fields = Vec::new();
    let mut effective_values = serde_json::Map::new();

    for (path, raw_value) in map {
        if !SUPPORTED_POLICY_PATHS.contains(&path.as_str()) {
            return Err(format!("Unsupported policy path: {path}"));
        }

        match raw_value {
            Value::Object(obj) => {
                let reason = obj
                    .get("reason")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                let effective = obj
                    .get("value")
                    .cloned()
                    .unwrap_or_else(|| Value::Object(obj.clone()));
                effective_values.insert(path.clone(), effective);
                enforced_fields.push(PolicyEnforcedField {
                    path: path.clone(),
                    reason,
                });
            }
            other => {
                effective_values.insert(path.clone(), other.clone());
                enforced_fields.push(PolicyEnforcedField {
                    path: path.clone(),
                    reason: None,
                });
            }
        }
    }

    Ok((enforced_fields, effective_values))
}

pub fn evaluate_policy_validity(policy: &PolicyState, now: DateTime<Utc>) -> bool {
    if matches!(policy.source, PolicySource::None) {
        return true;
    }

    if !policy.is_valid {
        return false;
    }

    match policy.expires_at {
        Some(expires_at) => expires_at >= now,
        None => true,
    }
}

pub fn normalize_policy_state(raw: Option<Value>, now: DateTime<Utc>) -> PolicyState {
    let Some(Value::Object(map)) = raw else {
        return PolicyState::default();
    };

    let source = parse_policy_source(map.get("source"));
    let eligible = map
        .get("eligible")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let active_policy_id = map
        .get("active_policy_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let active_version = map.get("active_version").and_then(|v| v.as_u64());
    let last_sync_at = parse_datetime(map.get("last_sync_at"));
    let last_success_at = parse_datetime(map.get("last_success_at"));
    let last_updated = parse_datetime(map.get("last_updated"));
    let expires_at = parse_datetime(map.get("expires_at"));
    let failure_reason = map
        .get("failure_reason")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let enforced_count = map
        .get("enforced_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let version = map
        .get("version")
        .and_then(|v| parse_policy_version(Some(v)));

    let enforced_fields = map
        .get("enforced_fields")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| {
                    let Value::Object(obj) = entry else {
                        return None;
                    };
                    let path = obj.get("path").and_then(|v| v.as_str())?.trim().to_string();
                    if path.is_empty() {
                        return None;
                    }
                    let reason = obj
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    Some(PolicyEnforcedField { path, reason })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let stored_validity = map
        .get("is_valid")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let mut state = PolicyState {
        source,
        eligible,
        is_valid: stored_validity,
        active_policy_id,
        active_version,
        last_sync_at,
        last_success_at,
        last_updated,
        expires_at,
        failure_reason,
        enforced_count,
        version,
        enforced_fields,
    };

    if state.enforced_count == 0 {
        state.enforced_count = state.enforced_fields.len() as u64;
    }

    state.is_valid = evaluate_policy_validity(&state, now);
    state
}

pub fn policy_state_for_command(raw: Option<Value>, now: DateTime<Utc>) -> PolicyState {
    normalize_policy_state(raw, now)
}

pub fn policy_state_for_sync_failure(
    current: &PolicyState,
    now: DateTime<Utc>,
    reason: &str,
) -> PolicyState {
    let has_cached_policy = current.version.is_some()
        || !current.enforced_fields.is_empty()
        || matches!(
            current.source,
            PolicySource::Cloud | PolicySource::Cached | PolicySource::DegradedExpired
        );

    let expired = current
        .expires_at
        .is_some_and(|expires_at| expires_at < now);

    let mut next = current.clone();
    next.eligible = true;
    next.last_sync_at = Some(now);
    next.failure_reason = Some(reason.to_string());

    if !has_cached_policy {
        next.source = PolicySource::None;
        next.is_valid = true;
        next.enforced_fields.clear();
        next.enforced_count = 0;
        return next;
    }

    if expired {
        next.source = PolicySource::DegradedExpired;
        next.is_valid = false;
    } else {
        next.source = PolicySource::Cached;
        next.is_valid = true;
    }

    next.enforced_count = next.enforced_fields.len() as u64;
    next
}

#[derive(Debug, Clone)]
pub struct PolicySyncOutcome {
    pub policy_state: PolicyState,
    pub effective_values: serde_json::Map<String, Value>,
}

pub fn validate_cloud_policy_candidate(
    candidate: &Value,
    current: &PolicyState,
    now: DateTime<Utc>,
) -> Result<PolicySyncOutcome, String> {
    let Value::Object(map) = candidate else {
        return Err("Policy payload must be a JSON object".to_string());
    };

    let version = parse_policy_version(map.get("version"))
        .ok_or_else(|| "Policy payload is missing version".to_string())?;

    if let (Some(next_num), Some(current_num)) = (
        parse_version_num(&version),
        current.version.as_deref().and_then(parse_version_num),
    ) {
        if next_num < current_num {
            return Err(format!(
                "Policy version regression: received {next_num} while current is {current_num}"
            ));
        }
    }

    let expires_at = parse_datetime(map.get("expires_at"));
    if map.contains_key("expires_at") && expires_at.is_none() {
        return Err("Policy expires_at must be RFC3339 when provided".to_string());
    }

    if let Some(expires) = expires_at {
        if expires < now {
            return Err("Policy candidate is already expired".to_string());
        }
    }

    let (enforced_fields, effective_values) = parse_constraints(map.get("constraints"))?;

    let active_policy_id = map
        .get("policy_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            map.get("policyId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

    let active_version = parse_version_num(&version);

    Ok(PolicySyncOutcome {
        policy_state: PolicyState {
            source: PolicySource::Cloud,
            eligible: true,
            is_valid: true,
            active_policy_id,
            active_version,
            last_sync_at: Some(now),
            last_success_at: Some(now),
            last_updated: Some(now),
            expires_at,
            failure_reason: None,
            enforced_count: enforced_fields.len() as u64,
            version: Some(version),
            enforced_fields,
        },
        effective_values,
    })
}

fn is_sensitive_policy_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.contains("api_key")
        || p.contains("token")
        || p.contains("password")
        || p.contains("secret")
        || p.contains("credential")
}

fn redact_effective_value(path: &str, value: Option<Value>) -> Option<Value> {
    if is_sensitive_policy_path(path) {
        return None;
    }
    value
}

pub fn build_policy_diagnostic_export(
    policy_state: PolicyState,
    now: DateTime<Utc>,
    effective_values: Option<&serde_json::Map<String, Value>>,
) -> PolicyDiagnosticExport {
    let enforced_fields = policy_state
        .enforced_fields
        .iter()
        .map(|field| PolicyDiagnosticField {
            path: field.path.clone(),
            effective_value: redact_effective_value(
                &field.path,
                effective_values
                    .and_then(|values| values.get(&field.path))
                    .cloned(),
            ),
            reason: field.reason.clone(),
        })
        .collect::<Vec<_>>();

    PolicyDiagnosticExport {
        generated_at: now,
        policy_state,
        enforced_fields,
        redaction_applied: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::{json, Value};

    #[test]
    fn normalize_policy_state_defaults_to_unmanaged() {
        let now = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let state = normalize_policy_state(None, now);
        assert_eq!(state.source, PolicySource::None);
        assert!(state.is_valid);
        assert!(!state.eligible);
        assert_eq!(state.enforced_count, 0);
        assert!(state.enforced_fields.is_empty());
    }

    #[test]
    fn expired_policy_is_marked_invalid() {
        let now = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let raw = json!({
            "source": "cloud",
            "is_valid": true,
            "expires_at": "2026-02-13T11:59:59Z"
        });
        let state = normalize_policy_state(Some(raw), now);
        assert_eq!(state.source, PolicySource::Cloud);
        assert!(!state.is_valid);
    }

    #[test]
    fn cloud_candidate_rejects_older_version() {
        let now = Utc.with_ymd_and_hms(2026, 2, 14, 10, 0, 0).unwrap();
        let current = PolicyState {
            version: Some("3".to_string()),
            ..PolicyState::default()
        };

        let candidate = json!({
            "version": 2,
            "constraints": {
                "rewrite_llm_enabled": true
            }
        });

        let result = validate_cloud_policy_candidate(&candidate, &current, now);
        assert!(result.is_err());
    }

    #[test]
    fn cloud_candidate_accepts_supported_constraints() {
        let now = Utc.with_ymd_and_hms(2026, 2, 14, 10, 0, 0).unwrap();
        let current = PolicyState::default();

        let candidate = json!({
            "policy_id": "policy-1",
            "version": "v2",
            "expires_at": "2026-03-01T00:00:00Z",
            "constraints": {
                "rewrite_llm_enabled": {
                    "value": true,
                    "reason": "Org required"
                },
                "request_logs_privacy_mode": true
            }
        });

        let result = validate_cloud_policy_candidate(&candidate, &current, now)
            .expect("candidate should be valid");

        assert_eq!(result.policy_state.source, PolicySource::Cloud);
        assert_eq!(
            result.policy_state.active_policy_id.as_deref(),
            Some("policy-1")
        );
        assert_eq!(result.policy_state.active_version, Some(2));
        assert_eq!(result.policy_state.enforced_count, 2);
        assert_eq!(
            result.effective_values.get("rewrite_llm_enabled"),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn sync_failure_uses_cached_policy_before_expiry() {
        let now = Utc.with_ymd_and_hms(2026, 2, 14, 12, 0, 0).unwrap();
        let current = PolicyState {
            source: PolicySource::Cloud,
            eligible: true,
            is_valid: true,
            active_policy_id: Some("policy-1".to_string()),
            active_version: Some(3),
            last_sync_at: None,
            last_success_at: Some(now),
            last_updated: Some(now),
            expires_at: Some(Utc.with_ymd_and_hms(2026, 2, 15, 12, 0, 0).unwrap()),
            failure_reason: None,
            enforced_count: 1,
            version: Some("3".to_string()),
            enforced_fields: vec![PolicyEnforcedField {
                path: "request_logs_privacy_mode".to_string(),
                reason: Some("Org policy".to_string()),
            }],
        };

        let next = policy_state_for_sync_failure(&current, now, "policy_sync_unavailable");
        assert_eq!(next.source, PolicySource::Cached);
        assert!(next.is_valid);
        assert_eq!(
            next.failure_reason.as_deref(),
            Some("policy_sync_unavailable")
        );
        assert_eq!(next.enforced_count, 1);
    }

    #[test]
    fn sync_failure_enters_degraded_when_cached_policy_expired() {
        let now = Utc.with_ymd_and_hms(2026, 2, 14, 12, 0, 0).unwrap();
        let current = PolicyState {
            source: PolicySource::Cached,
            eligible: true,
            is_valid: true,
            active_policy_id: Some("policy-1".to_string()),
            active_version: Some(3),
            last_sync_at: None,
            last_success_at: Some(now),
            last_updated: Some(now),
            expires_at: Some(Utc.with_ymd_and_hms(2026, 2, 14, 11, 59, 59).unwrap()),
            failure_reason: None,
            enforced_count: 1,
            version: Some("3".to_string()),
            enforced_fields: vec![PolicyEnforcedField {
                path: "request_logs_privacy_mode".to_string(),
                reason: Some("Org policy".to_string()),
            }],
        };

        let next = policy_state_for_sync_failure(&current, now, "policy_sync_unavailable");
        assert_eq!(next.source, PolicySource::DegradedExpired);
        assert!(!next.is_valid);
    }

    #[test]
    fn newer_policy_recovers_from_degraded_state() {
        let now = Utc.with_ymd_and_hms(2026, 2, 14, 12, 0, 0).unwrap();
        let current = PolicyState {
            source: PolicySource::DegradedExpired,
            eligible: true,
            is_valid: false,
            active_policy_id: Some("policy-1".to_string()),
            active_version: Some(3),
            last_sync_at: Some(now),
            last_success_at: Some(Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap()),
            last_updated: Some(Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap()),
            expires_at: Some(Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap()),
            failure_reason: Some("policy_sync_unavailable".to_string()),
            enforced_count: 1,
            version: Some("3".to_string()),
            enforced_fields: vec![PolicyEnforcedField {
                path: "request_logs_privacy_mode".to_string(),
                reason: Some("Org policy".to_string()),
            }],
        };

        let candidate = json!({
            "policy_id": "policy-2",
            "version": 4,
            "expires_at": "2026-03-01T00:00:00Z",
            "constraints": {
                "request_logs_privacy_mode": true
            }
        });

        let result = validate_cloud_policy_candidate(&candidate, &current, now)
            .expect("newer policy should recover degraded state");
        assert_eq!(result.policy_state.source, PolicySource::Cloud);
        assert!(result.policy_state.is_valid);
        assert_eq!(result.policy_state.active_version, Some(4));
    }

    #[test]
    fn diagnostics_redacts_sensitive_effective_values() {
        let now = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let policy_state = PolicyState {
            source: PolicySource::Cloud,
            eligible: true,
            is_valid: true,
            active_policy_id: Some("policy-1".to_string()),
            active_version: Some(1),
            last_sync_at: None,
            last_success_at: None,
            last_updated: None,
            expires_at: None,
            failure_reason: None,
            enforced_count: 2,
            version: Some("v1".to_string()),
            enforced_fields: vec![
                PolicyEnforcedField {
                    path: "request_logs_privacy_mode".to_string(),
                    reason: Some("Required".to_string()),
                },
                PolicyEnforcedField {
                    path: "openai_api_key".to_string(),
                    reason: Some("Never expose".to_string()),
                },
            ],
        };

        let mut effective = serde_json::Map::new();
        effective.insert("request_logs_privacy_mode".to_string(), Value::Bool(true));
        effective.insert(
            "openai_api_key".to_string(),
            Value::String("super-secret".to_string()),
        );

        let export = build_policy_diagnostic_export(policy_state, now, Some(&effective));

        let safe = export
            .enforced_fields
            .iter()
            .find(|x| x.path == "request_logs_privacy_mode")
            .and_then(|x| x.effective_value.clone());
        let secret = export
            .enforced_fields
            .iter()
            .find(|x| x.path == "openai_api_key")
            .and_then(|x| x.effective_value.clone());

        assert_eq!(safe, Some(Value::Bool(true)));
        assert_eq!(secret, None);
        assert!(export.redaction_applied);
    }
}
