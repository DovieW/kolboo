# Data Model: Phase 2 Cloud Policy Packs

## Entity: PolicyPack

- **Description**: Organization-scoped policy payload received from cloud and validated before use.
- **Fields**:
  - `policy_id` (string, required)
  - `org_id` (string, required)
  - `version` (integer, required, monotonic)
  - `issued_at` (ISO-8601 datetime, required)
  - `expires_at` (ISO-8601 datetime, required)
  - `constraints` (object, required) — allowed enforced keys and values
  - `defaults` (object, optional) — policy-provided default values
  - `signature` (string, required)
  - `key_id` (string, required)
- **Validation Rules**:
  - `expires_at > issued_at`
  - `version >= 1`
  - `constraints` keys must map to known setting fields
  - signature/key_id must verify against trusted keyset

## Entity: PolicyState

- **Description**: Current client-side policy source and lifecycle status.
- **Fields**:
  - `source` (enum: `none | cloud | cached | degraded_expired`)
  - `eligible` (boolean)
  - `active_policy_id` (string | null)
  - `active_version` (integer | null)
  - `last_sync_at` (ISO-8601 datetime | null)
  - `last_success_at` (ISO-8601 datetime | null)
  - `expires_at` (ISO-8601 datetime | null)
  - `failure_reason` (string | null)
  - `enforced_count` (integer)
- **State Transitions**:
  - `none -> cloud` on valid policy sync
  - `cloud -> cached` on temporary fetch failure before expiry
  - `cached -> degraded_expired` when expiry reached without refresh
  - `degraded_expired -> cloud` on next valid sync
  - `cloud/cached -> none` when user ineligible or signed out

## Entity: PolicyEnforcementRecord

- **Description**: Per-setting enforcement outcome after applying policy.
- **Fields**:
  - `setting_key` (string)
  - `status` (enum: `enforced | defaulted | rejected | editable`)
  - `effective_value` (json)
  - `reason` (string)
  - `policy_version` (integer | null)
- **Validation Rules**:
  - `status = enforced` requires `policy_version != null`
  - `rejected` entries must include non-empty `reason`

## Entity: PolicyDiagnosticsExport

- **Description**: Shareable support artifact for policy troubleshooting.
- **Fields**:
  - `generated_at` (ISO-8601 datetime)
  - `app_version` (string)
  - `policy_state` (PolicyState snapshot)
  - `policy_metadata` (policy_id, version, issued/expires timestamps)
  - `enforcement_summary` (counts by status)
  - `enforcement_records` (array of PolicyEnforcementRecord)
- **Validation Rules**:
  - Must exclude secrets/tokens/user content/transcripts
  - Must exclude raw auth headers and API keys

## Relationships

- One `PolicyPack` produces one active `PolicyState` snapshot at a time.
- One `PolicyPack` maps to many `PolicyEnforcementRecord` entries (one per setting key evaluated).
- One `PolicyDiagnosticsExport` embeds one `PolicyState` and many `PolicyEnforcementRecord` entries.
