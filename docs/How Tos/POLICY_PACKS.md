# Policy Packs (Cloud)

This guide explains how policy packs are consumed by the desktop app and how to debug policy behavior safely.

## What policy state means

- `none`: no active org policy is being enforced.
- `cloud`: latest policy was synced and applied successfully.
- `cached`: last known valid policy remains active while cloud sync is temporarily unavailable.
- `degraded_expired`: cached policy expired and refresh is still failing.

## Sync behavior

1. The app checks org/license eligibility.
2. If eligible, it attempts to read a cloud policy payload candidate.
3. Candidate validation enforces:
   - supported constraint keys only
   - monotonic version checks (no regression)
   - RFC3339 expiry validation
4. On success, effective values are persisted and applied to runtime settings.
5. The app emits:
   - `settings-changed`
   - `policy-state-changed`

## Enforcement behavior

- Enforced setting writes are blocked in the settings patch path.
- The UI renders a policy indicator and optional reason for locked controls.
- Controls become editable again when policy no longer enforces the field.

## Diagnostics export

Use the policy diagnostics export action from Settings.

The exported JSON includes:
- policy state metadata
- enforced field list
- redaction marker

The export **redacts sensitive values** for paths that look like secrets (API keys, tokens, passwords, credentials).

## Outage/degraded handling

- If sync fails and cached policy is still within expiry, source transitions to `cached`.
- If cached policy has expired and sync still fails, source transitions to `degraded_expired`.
- Next valid sync automatically recovers source to `cloud`.

## Troubleshooting quick checks

- Verify `license_state` indicates org eligibility.
- Inspect `policy_state` and `policy_effective_values` in `settings.json`.
- Watch for `policy-state-changed` and `settings-changed` events.
- Use diagnostics export for support handoff (redacted by design).
