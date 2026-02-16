# Phase 1 Data Model — Managed Inference Proxy

## Entity: InferenceModeSelection

- **Purpose**: Determines whether execution path is `managed` or `byok`.
- **Fields**:
  - `scope_type` (`user` | `org`)
  - `scope_id` (string)
  - `mode` (`managed` | `byok`)
  - `source` (`policy` | `entitlement` | `admin_override`)
  - `effective_at` (timestamp)
  - `version` (integer)
- **Validation**:
  - `mode` must be one of two allowed values.
  - Org-level version must monotonically increase.
- **State transitions**:
  - `managed -> byok` on admin policy change.
  - `byok -> managed` on admin policy change.

## Entity: ManagedEntitlementState

- **Purpose**: Represents whether subject is eligible for managed inference.
- **Fields**:
  - `subject_id` (string)
  - `tier` (`personal` | `enterprise` | `community`)
  - `eligible` (boolean)
  - `status` (`active` | `grace` | `expired` | `revoked`)
  - `valid_from` / `valid_to` (timestamps)
  - `org_id` (nullable string)
- **Validation**:
  - `eligible=true` requires tier that allows managed mode.
  - Current time must be within entitlement validity for managed acceptance.

## Entity: QuotaPolicy

- **Purpose**: Defines managed request ceilings and warning thresholds.
- **Fields**:
  - `policy_id` (string)
  - `subject_scope` (`user` | `org`)
  - `window` (`daily` | `monthly`)
  - `hard_limit` (integer)
  - `warning_thresholds` (array of percentage ints)
  - `rate_limit_profile` (string)
  - `effective_at` (timestamp)
- **Validation**:
  - `hard_limit > 0`
  - Thresholds unique, ascending, each in `[1,99]`

## Entity: UsageCounter

- **Purpose**: Tracks managed consumption against quota windows.
- **Fields**:
  - `scope_type` (`user` | `org`)
  - `scope_id` (string)
  - `metric` (`stt_seconds` | `llm_tokens` | `managed_requests`)
  - `window_start` / `window_end` (timestamps)
  - `used` (integer)
  - `updated_at` (timestamp)
- **Validation**:
  - `used >= 0`
  - Exactly one active row per (`scope`, `metric`, `window_start`).

## Entity: ManagedRequestRecord

- **Purpose**: Metadata-only lifecycle record for support and metering joins.
- **Fields**:
  - `request_id` (UUID)
  - `idempotency_key` (string)
  - `subject_id` (string)
  - `org_id` (nullable string)
  - `route` (`/v1/stt/transcribe` | `/v1/llm/complete`)
  - `mode` (`managed` | `byok`)
  - `status_class` (`accepted` | `unauthorized` | `ineligible` | `over_quota` | `temporarily_unavailable` | `upstream_failed`)
  - `provider` / `model` (string)
  - `started_at` / `ended_at` (timestamps)
  - `latency_ms` (integer)
- **Validation**:
  - No transcript/prompt/audio/OCR blobs persisted.
  - `idempotency_key` unique per (`subject_id`, `route`, logical request).

## Entity: MeteringLedgerEntry

- **Purpose**: Exactly-once accounting for managed usage.
- **Fields**:
  - `ledger_id` (UUID)
  - `idempotency_key` (string)
  - `request_id` (UUID)
  - `scope_type` / `scope_id`
  - `usage_delta` (integer)
  - `metric` (`stt_seconds` | `llm_tokens` | `managed_requests`)
  - `committed_at` (timestamp)
- **Validation**:
  - Unique constraint on (`scope_id`, `metric`, `idempotency_key`).
  - `usage_delta > 0` only for accepted managed requests.

## Entity: OrgKeyRotationState

- **Purpose**: Maintains enterprise BYOK continuity during key rotation.
- **Fields**:
  - `org_id` (string)
  - `active_key_version` (string)
  - `previous_key_version` (nullable string)
  - `rotation_started_at` / `rotation_completed_at` (timestamps)
  - `grace_period_until` (nullable timestamp)
- **Validation**:
  - Active version required.
  - During grace period, either key may validate.

## Relationships

- `InferenceModeSelection (org)` + `ManagedEntitlementState` determine effective execution mode.
- `QuotaPolicy` constrains `UsageCounter` updates.
- `ManagedRequestRecord` joins to `MeteringLedgerEntry` via `request_id` and `idempotency_key`.
- `OrgKeyRotationState` applies only when mode is `byok` for enterprise scope.
