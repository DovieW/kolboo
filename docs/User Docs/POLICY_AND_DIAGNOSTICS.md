# Policy and diagnostics

Kolboo includes an enterprise policy view in **Settings → Policy**.

## What you can see

- Policy status (active/invalid/unmanaged)
- Policy source (cloud, local file, unmanaged)
- Last updated / expiration timestamps
- Enforced fields and reasons

## Why some settings are locked

If your organization policy enforces a setting, Kolboo keeps that value compliant and ignores manual edits for that field.

That includes privacy-related controls such as product analytics in
**Settings → Data**. If your organization disables product analytics, the app
keeps that toggle off and shows the policy reason when available.

## Diagnostics export

Use **Export diagnostics** in the Policy tab to copy a redacted JSON report for support.

The export is designed to exclude sensitive values (API keys/tokens/secrets),
raw org names, and raw internal IDs.

The current support bundle includes:

- policy state metadata
- enforced field list
- app version + Sentry environment/release summary
- recent request IDs and support-safe request-log summaries
- support-safe hashed `user` / `org` targets so operators can correlate the
	bundle with the restricted operator console without needing raw identifiers in
	the exported file
