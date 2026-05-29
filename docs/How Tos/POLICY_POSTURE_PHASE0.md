# Policy posture (Phase 0)

This guide explains how the Phase 0 enterprise policy posture behaves in Kolboo.

## What Phase 0 does

- Keeps the app fully usable without login.
- Exposes a device policy state (`none | file | cloud`).
- Applies local policy validity checks (including expiry).
- Prevents edits to policy-enforced fields.
- Surfaces enforcement metadata in Settings → Policy.
- Provides a redacted diagnostics export for support.

## Enforcement behavior

When policy is active and valid:

- Enforced fields are treated as policy-controlled.
- User patch attempts on enforced fields are ignored.
- Runtime sync is triggered after policy-driven normalization/constraint handling.
- A `settings-changed` event is emitted with policy metadata payload fields:
  - `policy_normalized`
  - `policy_constraints_applied`
  - `policy_violations`

## Diagnostics export

Settings → Policy → **Export diagnostics** generates a redacted payload intended for support.

Included:

- policy source/status/timestamps/version
- enforced field list + reason
- redaction marker
- app version + Sentry release/environment summary
- recent request IDs and support-safe request-log summaries
- support-safe hashed `user` / `org` targets for restricted operator handoff

Excluded/redacted:

- API keys
- tokens / secrets / credentials
- transcript or audio content
- raw org names and raw internal IDs

## Troubleshooting quick checks

1. Open Settings → Policy and confirm status/source.
2. If status is invalid, check expiration metadata.
3. Attempt to edit a known enforced field and confirm it does not persist.
4. Export diagnostics and verify no sensitive values are present.
