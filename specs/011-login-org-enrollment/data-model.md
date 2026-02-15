# Data Model: Phase 1 Login and Org Enrollment

## Entity: LicenseState

### Fields
- `tier` (enum: `community | personal | enterprise`)
- `user_id` (string | null)
- `email` (string | null)
- `org_id` (string | null)
- `org_name` (string | null)
- `expires_at` (datetime | null)
- `cached_at` (datetime)
- `status` (enum: `signed_out | active | grace | expired`)
- `usage` (UsageStats)
- `limits` (TierLimits)

### State Transitions
- `signed_out -> active` on successful login + entitlement fetch.
- `active -> grace` when refresh fails but cache remains within grace window.
- `grace -> active` when refresh succeeds.
- `grace -> expired` when grace window elapses without refresh.
- `active|grace|expired -> signed_out` on logout.

## Entity: TierLimits

### Fields
- `stt_seconds_monthly` (integer)
- `llm_tokens_monthly` (integer)
- `requests_per_day` (integer)

### Validation Rules
- Values are non-negative integers.
- `community` may use zero values where managed limits are not applicable.

## Entity: UsageStats

### Fields
- `stt_seconds_used` (integer)
- `llm_tokens_used` (integer)
- `requests_today` (integer)

### Validation Rules
- Values are non-negative integers.
- Counters are content-free (no transcript/audio payloads).

## Entity: OrgEnrollment

### Fields
- `org_id` (string)
- `org_name` (string)
- `membership_state` (enum: `member | none`)

### Relationship
- `OrgEnrollment` is represented within `LicenseState` (`org_id`, `org_name`) and determines enterprise UI context.

## Entity: AuthSessionMaterial

### Fields
- `access_token` (secret string)
- `refresh_token` (secret string | null)
- `provider` (string)
- `issued_at` (datetime)

### Validation Rules
- Stored only in OS secure storage, never in `settings.json`.
- Never emitted in logs/events/diagnostics.
