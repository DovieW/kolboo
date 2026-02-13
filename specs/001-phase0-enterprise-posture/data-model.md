# Data Model: Phase 0 Enterprise Posture

## Entity: PolicyState

### Fields
- `source` (enum: `none | file | cloud`)
  - Origin of active policy.
- `last_updated` (datetime | null)
  - Last successful policy application time.
- `expires_at` (datetime | null)
  - Policy validity horizon if applicable.
- `is_valid` (boolean)
  - Whether the policy payload is currently valid.
- `version` (string | null)
  - Policy version identifier.

### State Transitions
- `none -> file/cloud` when a valid policy is loaded.
- `file/cloud -> none` when policy is removed/reset.
- `valid -> invalid` when policy fails validation/expiry checks.

## Entity: PolicyRuleSet

### Fields
- `allowed_stt_providers` (string[] | null)
- `allowed_llm_providers` (string[] | null)
- `allowed_models_by_provider` (record<string, string[]> | null)
- `require_request_logs_privacy_mode` (boolean | null)
- `disable_clipboard_context` (boolean | null)
- `disable_selection_probe` (boolean | null)
- `disable_save_recordings` (boolean | null)
- `force_proxy_settings` (object | null)
- `max_saved_recordings` (number | null)

### Validation Rules
- Missing/null fields imply no constraint unless explicitly documented otherwise.
- Unknown fields are ignored (forward compatibility) but may be surfaced in diagnostics.
- Invalid field types invalidate the policy payload.

## Entity: EffectiveSettings

### Fields
- Full normalized application settings after merge of user preferences + policy constraints.
- Includes enforcement metadata per constrained field (e.g., `enforced: true`, `reason`).

### Relationship
- Derived from `PolicyRuleSet` + user settings.
- Drives runtime sync and UI lock indicators.

## Entity: PolicyDiagnosticExport

### Fields
- `policy_state` (source/validity/timestamps/version)
- `enforced_fields` (list with effective value + rationale)
- `validation_outcomes` (pass/fail details)
- `generated_at` (datetime)

### Redaction Rules
- MUST exclude API keys, auth tokens, credentials, and secret headers.
- MUST exclude transcript/audio/prompt content.

## Behavioral Rules

1. Policy constraints always take precedence over user edits for constrained fields.
2. Unconstrained fields remain user-editable.
3. Any change in effective settings from policy application triggers sync/event propagation.
