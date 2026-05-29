# Telemetry Governance (Kolboo desktop)

Last updated: 2026-05-29

This document describes the current launch posture for desktop telemetry in `kolboo`.
It is meant to stay code-grounded: when telemetry behavior changes, update this doc in
the same PR so privacy copy, operator expectations, and support workflows do not drift.

## Scope split

Kolboo currently treats telemetry as two separate concerns:

- **PostHog product analytics**
  - answers coarse product-behavior questions
  - must remain event-only
  - must not include transcripts, prompts, completions, audio, OCR payloads,
    clipboard contents, or secrets
- **Sentry crash/error telemetry**
  - answers reliability questions
  - should capture failures, not user content
  - see `docs/Dev Docs/SENTRY_INTEGRATION.md` for the current Sentry posture

Launch-scope note:

- Product analytics governance in this document currently applies to the desktop
	app surface only.
- The private admin dashboard does **not** currently send PostHog product
	analytics; its launch-hardening telemetry scope is browser Sentry only when a
	DSN is configured.

## Desktop product analytics posture

Current desktop rules:

- No PostHog event is sent until the first-run telemetry disclosure is resolved.
- The local user can disable analytics in **Settings → Data**.
- Organization policy can force analytics off via the provider-neutral
  `disable_product_analytics` policy constraint.
- Desktop session replay and desktop autocapture stay off.
- The transport must re-read the current settings state for every event so opt-out
  takes effect immediately.
- There is intentionally no trailing `analytics_opted_out` event after a user
  disables analytics.

Code references:

- Disclosure contract: `app/src/lib/settings/telemetryDisclosure.ts`
- Product analytics transport: `app/src/lib/telemetry/posthog.ts`
- Data Settings orchestration: `app/src/lib/settings/dataBackupCloudSync.ts`
- Data Settings UI: `app/src/components/settings/data/DataCloudSyncSection.tsx`
- First-run disclosure modal: `app/src/components/settings/TelemetryDisclosureModal.tsx`
- Enterprise policy aliasing: `app/src/lib/tauri/settings.ts`
- Backend policy validation: `app/src-tauri/src/policy.rs`
- Backend policy application: `app/src-tauri/src/commands/policy.rs`

## Current desktop event taxonomy

As of this document, desktop product analytics is intentionally tiny.

| Event                          | Trigger                                         | Allowed properties     | Source                                        |
| ------------------------------ | ----------------------------------------------- | ---------------------- | --------------------------------------------- |
| `cloud_sync_action_succeeded`  | Manual cloud-sync push/pull succeeds            | `action`               | `app/src/lib/settings/dataBackupCloudSync.ts` |
| `cloud_sync_action_failed`     | Manual cloud-sync push/pull fails               | `action`, `error_kind` | `app/src/lib/settings/dataBackupCloudSync.ts` |
| `cloud_sync_enabled_changed`   | User toggles cloud sync                         | `enabled`              | `app/src/lib/settings/dataBackupCloudSync.ts` |
| `cloud_sync_auto_push_changed` | User toggles cloud-sync auto-push               | `enabled`              | `app/src/lib/settings/dataBackupCloudSync.ts` |
| `analytics_opted_in`           | User explicitly enables analytics from Settings | `surface`              | `app/src/lib/settings/dataBackupCloudSync.ts` |

If a new event is added, update this table and the user-facing privacy wording in
`docs/User Docs/PRIVACY_AND_DATA.md`.

## Identifier and local-state surfaces

Desktop analytics currently persists a small amount of local state:

- `settings.json`
  - `posthog_analytics_enabled`
  - `telemetry_disclosure_acknowledged_at`
  - `telemetry_disclosure_version`
- local storage
  - `kolboo_posthog_distinct_id_v1`

The distinct ID is a locally generated random identifier used for coarse product
analytics. It is not derived from transcript text or provider credentials.

## Redaction and payload limits

The current PostHog transport sanitizes event properties before sending them.
The sanitizer currently redacts or blocks:

- transcript / text / prompt / completion fields
- audio / wav fields
- OCR fields
- clipboard fields
- API keys, tokens, secrets, passwords, cookies, authorization headers, and
  bearer-style strings
- JWT-like strings

Additional limits:

- arrays are truncated to 20 entries
- long strings are truncated to 256 characters

These limits are enforced in `app/src/lib/telemetry/posthog.ts` and covered by
`app/src/lib/telemetry/posthog.test.ts`.

## Retention and deletion notes

Desktop code does **not** keep a separate local archive of product analytics
payloads. The local persistence surface is only the disclosure/toggle settings plus
the local distinct ID.

Operational notes:

- Clearing app settings/app storage can reset the local analytics disclosure state
  and distinct ID.
- Server-side retention for PostHog events is managed in the PostHog project / ops
  configuration rather than in this repo.
- Private-repo env/ops references for the current PostHog host and project
	configuration live in `kolboo-private/docs/SECRETS_AND_ENVIRONMENTS.md`.
- If retention policy changes materially, update this doc and any operator runbooks
  in the same change.

## Support-safe correlation guidance

When debugging telemetry-related issues:

- prefer redacted diagnostics exports and structured error categories
- the current policy diagnostics export now includes request IDs plus hashed
  `user` / `org` correlation targets specifically so support can join desktop
  evidence to the restricted operator surfaces without embedding raw IDs in the
  exported file
- do not ask users for raw transcripts, prompts, clipboard contents, or API keys
  just to explain a telemetry event
- treat the local PostHog distinct ID as a coarse product-analytics identifier,
  not as proof of user identity
- if cross-system correlation rules broaden beyond the current posture, document
  the support-safe identifier strategy explicitly before rollout

Code references for the support bundle:

- `app/src/components/settings/PolicySettings.tsx`
- `app/src/components/settings/policyDiagnostics.ts`

## Update checklist

When changing telemetry behavior, update this doc if any of the following changes:

- event names or allowed properties
- disclosure timing / default behavior
- policy-enforced analytics behavior
- replay/autocapture posture
- local persistence surfaces or identifier behavior
- redaction rules or truncation limits
