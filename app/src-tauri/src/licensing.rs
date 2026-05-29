use chrono::{DateTime, Duration, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(desktop)]
use tauri::AppHandle;

pub const DEFAULT_GRACE_DAYS: i64 = 7;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LicenseTier {
    #[default]
    Community,
    Personal,
    Enterprise,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LicenseStatus {
    #[default]
    SignedOut,
    Active,
    Grace,
    Expired,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct TierLimits {
    pub stt_seconds_monthly: u64,
    pub llm_tokens_monthly: u64,
    pub requests_per_day: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct UsageStats {
    pub stt_seconds_used: u64,
    pub llm_tokens_used: u64,
    pub requests_today: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrgInferenceMode {
    OrgByok,
    Managed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct OrgContext {
    pub org_id: String,
    pub org_name: String,
    pub inference_mode: Option<OrgInferenceMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LicenseState {
    pub tier: LicenseTier,
    pub status: LicenseStatus,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub org: Option<OrgContext>,
    pub expires_at: Option<DateTime<Utc>>,
    pub cached_at: DateTime<Utc>,
    pub last_validated_at: Option<DateTime<Utc>>,
    pub usage: UsageStats,
    pub limits: TierLimits,
    pub portal_available: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthReasonCode {
    ReauthRequired,
    TokenInvalid,
    MembershipMissing,
    InsufficientTier,
    PolicyDenied,
    AuthNotConfigured,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyStatus {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LicenseAuthContext {
    pub authenticated: bool,
    pub secure_session_present: bool,
    pub subject_id: Option<String>,
    pub issuer: Option<String>,
    pub mode: LicenseTier,
    pub org_id: Option<String>,
    pub entitlements: Vec<String>,
    pub policy_status: PolicyStatus,
    pub reason_code: Option<AuthReasonCode>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenExchangeDecision {
    DirectIdpToken,
    AdoptTokenExchange,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct TokenExchangeTriggerSet {
    pub multi_idp_required: bool,
    pub kill_switch_required: bool,
    pub embedded_claims_required: bool,
    pub desktop_idp_agnostic_required: bool,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub decision: TokenExchangeDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SessionExchangeRequest {
    pub upstream_access_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SessionExchangeResponse {
    pub enabled: bool,
    pub decision: TokenExchangeDecision,
    pub trigger_set: TokenExchangeTriggerSet,
    pub session_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub claims: Value,
    pub reason: String,
}

impl LicenseState {
    pub fn signed_out(now: DateTime<Utc>) -> Self {
        Self {
            tier: LicenseTier::Community,
            status: LicenseStatus::SignedOut,
            user_id: None,
            email: None,
            org: None,
            expires_at: None,
            cached_at: now,
            last_validated_at: None,
            usage: UsageStats::default(),
            limits: TierLimits::default(),
            portal_available: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LoginRequest {
    pub provider_hint: Option<String>,
    pub auth_provider: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SignupResponse {
    pub state: LicenseState,
    pub confirmation_required: bool,
    pub email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMaterial {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LicenseTransition {
    pub from: LicenseStatus,
    pub to: LicenseStatus,
    pub occurred_at: DateTime<Utc>,
    pub reason: String,
}

pub fn build_transition(
    from: LicenseStatus,
    to: LicenseStatus,
    occurred_at: DateTime<Utc>,
    reason: impl Into<String>,
) -> LicenseTransition {
    LicenseTransition {
        from,
        to,
        occurred_at,
        reason: reason.into(),
    }
}

fn redact_identifier(value: Option<&str>) -> String {
    let Some(raw) = value else {
        return "none".to_string();
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "none".to_string();
    }

    format!("present:{}", trimmed.len())
}

pub fn telemetry_context_for_state(state: &LicenseState) -> Value {
    serde_json::json!({
        "tier": state.tier,
        "status": state.status,
        "user_id": redact_identifier(state.user_id.as_deref()),
        "email": redact_identifier(state.email.as_deref()),
        "has_org": state.org.is_some(),
        "org_id": redact_identifier(state.org.as_ref().map(|org| org.org_id.as_str())),
        "org_name": redact_identifier(state.org.as_ref().map(|org| org.org_name.as_str())),
        "org_inference_mode": state.org.as_ref().and_then(|org| serde_json::to_value(org.inference_mode).ok()),
        "has_expires_at": state.expires_at.is_some(),
        "has_last_validated_at": state.last_validated_at.is_some(),
        "portal_available": state.portal_available,
    })
}

#[cfg(desktop)]
pub fn persist_session_material(app: &AppHandle, session: &SessionMaterial) -> Result<(), String> {
    crate::secrets::persist_auth_session_material(
        app,
        &session.access_token,
        &session.refresh_token,
    )
}

#[cfg(not(desktop))]
pub fn persist_session_material(
    _app: &tauri::AppHandle,
    _session: &SessionMaterial,
) -> Result<(), String> {
    Ok(())
}

#[cfg(desktop)]
pub fn load_session_material(app: &AppHandle) -> Option<SessionMaterial> {
    let session = crate::secrets::load_auth_session_material(app)?;
    Some(SessionMaterial {
        access_token: session.access_token,
        refresh_token: session.refresh_token,
    })
}

pub fn evaluate_token_exchange_decision(
    trigger_set: &TokenExchangeTriggerSet,
) -> TokenExchangeDecision {
    if trigger_set.multi_idp_required
        || trigger_set.kill_switch_required
        || trigger_set.embedded_claims_required
        || trigger_set.desktop_idp_agnostic_required
    {
        TokenExchangeDecision::AdoptTokenExchange
    } else {
        TokenExchangeDecision::DirectIdpToken
    }
}

pub fn normalize_token_exchange_trigger_set(
    raw: Option<Value>,
    now: DateTime<Utc>,
) -> TokenExchangeTriggerSet {
    let Some(Value::Object(map)) = raw else {
        return TokenExchangeTriggerSet {
            multi_idp_required: false,
            kill_switch_required: false,
            embedded_claims_required: false,
            desktop_idp_agnostic_required: false,
            reviewed_at: None,
            decision: TokenExchangeDecision::DirectIdpToken,
        };
    };

    let mut trigger_set = TokenExchangeTriggerSet {
        multi_idp_required: map
            .get("multi_idp_required")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        kill_switch_required: map
            .get("kill_switch_required")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        embedded_claims_required: map
            .get("embedded_claims_required")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        desktop_idp_agnostic_required: map
            .get("desktop_idp_agnostic_required")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        reviewed_at: parse_datetime(map.get("reviewed_at")),
        decision: TokenExchangeDecision::DirectIdpToken,
    };

    if trigger_set.reviewed_at.is_none() && map.contains_key("reviewed_at") {
        trigger_set.reviewed_at = Some(now);
    }

    trigger_set.decision = evaluate_token_exchange_decision(&trigger_set);
    trigger_set
}

#[cfg(not(desktop))]
pub fn load_session_material(_app: &tauri::AppHandle) -> Option<SessionMaterial> {
    None
}

#[cfg(desktop)]
pub fn clear_session_material(app: &AppHandle) -> Result<(), String> {
    crate::secrets::clear_auth_session_material(app)
}

#[cfg(not(desktop))]
pub fn clear_session_material(_app: &tauri::AppHandle) -> Result<(), String> {
    Ok(())
}

fn parse_datetime(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let raw = value.and_then(|x| x.as_str())?;
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_tier(value: Option<&Value>) -> LicenseTier {
    match value.and_then(|x| x.as_str()) {
        Some("enterprise") => LicenseTier::Enterprise,
        Some("personal") => LicenseTier::Personal,
        _ => LicenseTier::Community,
    }
}

fn parse_status(value: Option<&Value>) -> LicenseStatus {
    match value.and_then(|x| x.as_str()) {
        Some("active") => LicenseStatus::Active,
        Some("grace") => LicenseStatus::Grace,
        Some("expired") => LicenseStatus::Expired,
        _ => LicenseStatus::SignedOut,
    }
}

fn parse_u64(value: Option<&Value>) -> u64 {
    value.and_then(|v| v.as_u64()).unwrap_or(0)
}

fn parse_org_inference_mode(value: Option<&Value>) -> Option<OrgInferenceMode> {
    match value.and_then(|x| x.as_str()) {
        Some("org_byok") => Some(OrgInferenceMode::OrgByok),
        Some("managed") => Some(OrgInferenceMode::Managed),
        _ => None,
    }
}

fn tier_limits_for(tier: LicenseTier) -> TierLimits {
    match tier {
        LicenseTier::Community => TierLimits::default(),
        LicenseTier::Personal => TierLimits {
            stt_seconds_monthly: 18_000,
            llm_tokens_monthly: 500_000,
            requests_per_day: 500,
        },
        LicenseTier::Enterprise => TierLimits {
            stt_seconds_monthly: 120_000,
            llm_tokens_monthly: 2_000_000,
            requests_per_day: 5_000,
        },
    }
}

fn grace_expires_at(
    last_validated_at: Option<DateTime<Utc>>,
    grace_days: i64,
) -> Option<DateTime<Utc>> {
    last_validated_at.map(|ts| ts + Duration::days(grace_days))
}

pub fn evaluate_status(state: &LicenseState, now: DateTime<Utc>, grace_days: i64) -> LicenseStatus {
    if matches!(state.status, LicenseStatus::SignedOut)
        || state
            .user_id
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        return LicenseStatus::SignedOut;
    }

    // A signed-in community account is still a real authenticated desktop session.
    // We keep it active so BYOK/local usage remains available even when no managed
    // entitlement rows exist yet or the hydration path is temporarily unavailable.
    if matches!(state.tier, LicenseTier::Community) {
        return LicenseStatus::Active;
    }

    let not_expired = state
        .expires_at
        .map(|expires_at| expires_at >= now)
        .unwrap_or(false);

    if not_expired {
        return LicenseStatus::Active;
    }

    let in_grace = grace_expires_at(state.last_validated_at, grace_days)
        .map(|deadline| deadline >= now)
        .unwrap_or(false);

    if in_grace {
        LicenseStatus::Grace
    } else {
        LicenseStatus::Expired
    }
}

pub fn normalize_license_state(raw: Option<Value>, now: DateTime<Utc>) -> LicenseState {
    let Some(Value::Object(map)) = raw else {
        return LicenseState::signed_out(now);
    };

    let tier = parse_tier(map.get("tier"));
    let mut state = LicenseState {
        tier,
        status: parse_status(map.get("status")),
        user_id: map
            .get("user_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        email: map
            .get("email")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        org: map.get("org").and_then(|v| v.as_object()).and_then(|org| {
            let org_id = org.get("org_id")?.as_str()?.trim().to_string();
            let org_name = org.get("org_name")?.as_str()?.trim().to_string();
            if org_id.is_empty() || org_name.is_empty() {
                return None;
            }
            Some(OrgContext {
                org_id,
                org_name,
                inference_mode: parse_org_inference_mode(org.get("inference_mode")),
            })
        }),
        expires_at: parse_datetime(map.get("expires_at")),
        cached_at: parse_datetime(map.get("cached_at")).unwrap_or(now),
        last_validated_at: parse_datetime(map.get("last_validated_at")),
        usage: map
            .get("usage")
            .and_then(|v| v.as_object())
            .map(|usage| UsageStats {
                stt_seconds_used: parse_u64(usage.get("stt_seconds_used")),
                llm_tokens_used: parse_u64(usage.get("llm_tokens_used")),
                requests_today: parse_u64(usage.get("requests_today")),
            })
            .unwrap_or_default(),
        limits: map
            .get("limits")
            .and_then(|v| v.as_object())
            .map(|limits| TierLimits {
                stt_seconds_monthly: parse_u64(limits.get("stt_seconds_monthly")),
                llm_tokens_monthly: parse_u64(limits.get("llm_tokens_monthly")),
                requests_per_day: parse_u64(limits.get("requests_per_day")),
            })
            .unwrap_or_else(|| tier_limits_for(tier)),
        portal_available: map
            .get("portal_available")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    };

    if matches!(state.tier, LicenseTier::Community) {
        state.org = None;
    }

    state.status = evaluate_status(&state, now, DEFAULT_GRACE_DAYS);
    state
}

pub fn build_login_state(provider_hint: Option<&str>, now: DateTime<Utc>) -> LicenseState {
    let _ = provider_hint;

    // We intentionally do not infer plan/org access from the provider hint anymore.
    // Real access must come back from api-edge so the desktop does not fabricate
    // enterprise/personal state before the durable backend confirms it.
    LicenseState {
        tier: LicenseTier::Community,
        status: LicenseStatus::Active,
        user_id: None,
        email: None,
        org: None,
        expires_at: None,
        cached_at: now,
        last_validated_at: Some(now),
        usage: UsageStats::default(),
        limits: tier_limits_for(LicenseTier::Community),
        portal_available: false,
    }
}

pub fn apply_refresh_success(mut state: LicenseState, now: DateTime<Utc>) -> LicenseState {
    state.status = LicenseStatus::Active;
    state.cached_at = now;
    state.last_validated_at = Some(now);
    state.expires_at = if matches!(state.tier, LicenseTier::Community) {
        None
    } else {
        Some(now + Duration::days(30))
    };
    state
}

pub fn apply_refresh_failure(mut state: LicenseState, now: DateTime<Utc>) -> LicenseState {
    state.cached_at = now;
    state.status = evaluate_status(&state, now, DEFAULT_GRACE_DAYS);
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    #[test]
    fn normalizes_signed_out_when_missing() {
        let now = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let state = normalize_license_state(None, now);
        assert_eq!(state.status, LicenseStatus::SignedOut);
        assert_eq!(state.tier, LicenseTier::Community);
    }

    #[test]
    fn maps_org_context_for_enterprise_login() {
        let now = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let state = build_login_state(Some("enterprise"), now);
        assert_eq!(state.tier, LicenseTier::Community);
        assert!(state.org.is_none());
        assert_eq!(state.status, LicenseStatus::Active);
        assert!(!state.portal_available);
    }

    #[test]
    fn signed_in_community_state_stays_active() {
        let now = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let mut state = build_login_state(None, now);
        state.user_id = Some("user-123".to_string());

        assert_eq!(
            evaluate_status(&state, now, DEFAULT_GRACE_DAYS),
            LicenseStatus::Active
        );
    }

    #[test]
    fn transitions_active_to_grace_to_expired() {
        let base = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let raw = json!({
            "tier": "personal",
            "status": "active",
            "user_id": "u1",
            "email": "u1@example.com",
            "expires_at": "2026-02-12T00:00:00Z",
            "last_validated_at": "2026-02-10T12:00:00Z",
            "cached_at": "2026-02-12T00:00:00Z"
        });

        let state_grace = normalize_license_state(Some(raw.clone()), base);
        assert_eq!(state_grace.status, LicenseStatus::Grace);

        let after_grace = Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 1).unwrap();
        let state_expired = normalize_license_state(Some(raw), after_grace);
        assert_eq!(state_expired.status, LicenseStatus::Expired);
    }

    #[test]
    fn refresh_success_resets_paid_state_to_active() {
        let now = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let mut state = LicenseState::signed_out(now);
        state.tier = LicenseTier::Personal;
        state.user_id = Some("u1".to_string());
        state.email = Some("u1@example.com".to_string());
        state.status = LicenseStatus::Expired;

        let next = apply_refresh_success(state, now);
        assert_eq!(next.status, LicenseStatus::Active);
        assert!(next.expires_at.is_some());
        assert!(next.last_validated_at.is_some());
    }

    #[test]
    fn refresh_success_keeps_signed_in_community_without_expiry() {
        let now = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let mut state = build_login_state(None, now);
        state.user_id = Some("u1".to_string());
        state.email = Some("u1@example.com".to_string());
        state.status = LicenseStatus::Grace;

        let next = apply_refresh_success(state, now);
        assert_eq!(next.status, LicenseStatus::Active);
        assert!(next.expires_at.is_none());
        assert!(next.last_validated_at.is_some());
    }

    #[test]
    fn refresh_failure_keeps_paid_state_in_grace_when_window_valid() {
        let now = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let mut state = LicenseState::signed_out(now);
        state.tier = LicenseTier::Personal;
        state.user_id = Some("u1".to_string());
        state.email = Some("u1@example.com".to_string());
        state.status = LicenseStatus::Active;
        state.expires_at = Some(now - Duration::hours(1));
        state.last_validated_at = Some(now - Duration::days(1));

        let next = apply_refresh_failure(state, now);
        assert_eq!(next.status, LicenseStatus::Grace);
    }

    #[test]
    fn refresh_failure_keeps_signed_in_community_active() {
        let now = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let mut state = build_login_state(None, now);
        state.user_id = Some("u1".to_string());
        state.email = Some("u1@example.com".to_string());
        state.status = LicenseStatus::Grace;
        state.expires_at = Some(now - Duration::days(30));
        state.last_validated_at = Some(now - Duration::days(30));

        let next = apply_refresh_failure(state, now);
        assert_eq!(next.status, LicenseStatus::Active);
    }

    #[test]
    fn telemetry_context_redacts_sensitive_fields() {
        let now = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let state = LicenseState {
            tier: LicenseTier::Enterprise,
            status: LicenseStatus::Active,
            user_id: Some("user-secret-abc123".to_string()),
            email: Some("sensitive@example.com".to_string()),
            org: Some(OrgContext {
                org_id: "org-sensitive".to_string(),
                org_name: "Confidential Org".to_string(),
                inference_mode: Some(OrgInferenceMode::Managed),
            }),
            expires_at: Some(now + Duration::days(7)),
            cached_at: now,
            last_validated_at: Some(now),
            usage: UsageStats::default(),
            limits: TierLimits::default(),
            portal_available: false,
        };

        let payload = telemetry_context_for_state(&state);
        assert_eq!(
            payload.get("user_id").and_then(|v| v.as_str()),
            Some("present:18")
        );
        assert_eq!(
            payload.get("email").and_then(|v| v.as_str()),
            Some("present:21")
        );
        assert_eq!(
            payload.get("org_id").and_then(|v| v.as_str()),
            Some("present:13")
        );
        assert_eq!(
            payload.get("org_name").and_then(|v| v.as_str()),
            Some("present:16")
        );
        assert_eq!(payload.get("has_org").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn normalizes_hydrated_org_inference_mode_and_portal_availability() {
        let now = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let state = normalize_license_state(
            Some(json!({
                "tier": "enterprise",
                "status": "active",
                "user_id": "user-123",
                "email": "user@example.com",
                "org": {
                    "org_id": "org-123",
                    "org_name": "Kolboo Shared Dev Pilot Org",
                    "inference_mode": "org_byok"
                },
                "cached_at": "2026-02-13T12:00:00Z",
                "last_validated_at": "2026-02-13T12:00:00Z",
                "portal_available": false,
                "limits": {
                    "stt_seconds_monthly": 0,
                    "llm_tokens_monthly": 0,
                    "requests_per_day": 0
                }
            })),
            now,
        );

        assert_eq!(
            state.org.as_ref().map(|org| org.org_name.as_str()),
            Some("Kolboo Shared Dev Pilot Org")
        );
        assert_eq!(
            state.org.as_ref().and_then(|org| org.inference_mode),
            Some(OrgInferenceMode::OrgByok)
        );
        assert!(!state.portal_available);
    }

    #[test]
    fn token_exchange_decision_stays_direct_when_no_triggers_are_active() {
        let now = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let trigger_set = normalize_token_exchange_trigger_set(None, now);

        assert_eq!(trigger_set.decision, TokenExchangeDecision::DirectIdpToken);
    }

    #[test]
    fn token_exchange_decision_adopts_when_any_trigger_is_active() {
        let now = Utc.with_ymd_and_hms(2026, 2, 13, 12, 0, 0).unwrap();
        let trigger_set = normalize_token_exchange_trigger_set(
            Some(json!({
                "multi_idp_required": false,
                "kill_switch_required": true,
                "embedded_claims_required": false,
                "desktop_idp_agnostic_required": false,
                "reviewed_at": "2026-02-12T09:30:00Z",
                "decision": "direct_idp_token"
            })),
            now,
        );

        assert_eq!(
            trigger_set.decision,
            TokenExchangeDecision::AdoptTokenExchange
        );
        assert_eq!(
            trigger_set.reviewed_at,
            Some(Utc.with_ymd_and_hms(2026, 2, 12, 9, 30, 0).unwrap())
        );
    }
}
