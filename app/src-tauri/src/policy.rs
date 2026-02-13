use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PolicySource {
    #[default]
    None,
    File,
    Cloud,
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
    pub is_valid: bool,
    pub last_updated: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub version: Option<String>,
    #[serde(default)]
    pub enforced_fields: Vec<PolicyEnforcedField>,
}

impl Default for PolicyState {
    fn default() -> Self {
        Self {
            source: PolicySource::None,
            is_valid: true,
            last_updated: None,
            expires_at: None,
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
        _ => PolicySource::None,
    }
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
    let last_updated = parse_datetime(map.get("last_updated"));
    let expires_at = parse_datetime(map.get("expires_at"));
    let version = map
        .get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

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
        is_valid: stored_validity,
        last_updated,
        expires_at,
        version,
        enforced_fields,
    };

    state.is_valid = evaluate_policy_validity(&state, now);
    state
}

pub fn policy_state_for_command(raw: Option<Value>, now: DateTime<Utc>) -> PolicyState {
    normalize_policy_state(raw, now)
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
    fn diagnostics_redacts_sensitive_effective_values() {
        let now = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let policy_state = PolicyState {
            source: PolicySource::Cloud,
            is_valid: true,
            last_updated: None,
            expires_at: None,
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
