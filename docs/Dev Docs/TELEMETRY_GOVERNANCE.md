# Telemetry Governance (Kolboo desktop)

Last updated: 2026-09-24

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

- No PostHog event is sent until the current first-run telemetry disclosure is resolved.
- Basic usage events are always on after disclosure when PostHog is configured.
  Each event receives a fresh, unrelated ID and no person profile; there is no
  persistent device, account, session, or recording ID on this stream. It cannot
  measure unique users, active installs, funnels, or retention.
- **Settings → Data** controls optional linked analytics. When enabled, basic
  events also use the linked ID; additional cloud-sync/settings events remain
  opt-in. Signed-in events use the opaque Kolboo subject UUID and signed-out
  events use a random installation ID. No email or PostHog person profile is sent,
  and the two IDs are not merged. Re-review of the v2 disclosure is mandatory
  before any linked event may send. Disabling the toggle returns basic events to
  fresh per-event IDs immediately.
- Organization policy can force both streams off via the provider-neutral
  `disable_product_analytics` policy constraint.
- Desktop session replay and desktop autocapture stay off.
- The transport re-reads disclosure and policy for every event, plus the local
  toggle for optional events. Missing settings access fails closed.
- There is intentionally no trailing `analytics_opted_out` event after a user
  disables optional analytics.
- The app only accepts two hard-coded event allowlists. Basic properties are
  projected to known page names and bounded recording fields; unknown/custom
  provider and model names are grouped as `other`. Both streams set
  `$process_person_profile` to false and disable GeoIP enrichment. PostHog still
  receives the network request, including the source IP at its edge.

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

Basic, unlinked events:

| Event                            | Trigger                                             | Properties |
| -------------------------------- | --------------------------------------------------- | ---------- |
| `desktop_opened`                 | Main window opens after disclosure is reviewed      | None       |
| `page_viewed`                    | Active main view changes                            | `page` from fixed sidebar view names |
| `recording_started`              | Native recording-start event reaches main view     | None |
| `recording_finished`             | New/transitioned terminal History item is observed | `status`, `mode`, rounded `duration_seconds` (0–14,400), known `stt_provider`, known `stt_model` |
| `pipeline_transcription_started` | Native transcription-start event reaches main view | None       |
| `pipeline_transcript_ready`      | Native ready event has non-empty text               | None       |
| `pipeline_error`                 | Native pipeline error event reaches main view       | None       |

These are best-effort counts, not guaranteed unique recording outcomes. The
recording observer compares the first 50 recent History summaries to a baseline;
it never sends history IDs, previews or error messages. It can miss an outcome
when the main view is closed or more than 50 items arrive between observations.
Model IDs are sent only if present in the bundled, code-reviewed STT catalog;
dynamic/custom values become `other`. Page views include programmatic navigation.

Optional, linked events:

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

Desktop analytics persists a small amount of local state:

- `settings.json`
  - `posthog_analytics_enabled`
  - `telemetry_disclosure_acknowledged_at`
  - `telemetry_disclosure_version`
- local storage
  - `kolboo_posthog_distinct_id_v1`

The random distinct ID is used for linked analytics while signed out. When signed
in and opted in, the authenticated subject UUID replaces it for new events only.
No `identify`/alias call merges previously unlinked or signed-out events.
When linked analytics are off, basic events do not read or write this ID.

## Payload limits

The transport accepts only the event names above. Basic events project caller
input onto fixed page and recording fields. Optional events project caller input onto event-specific categorical
fields (`action`, `error_kind`, `enabled`, or `surface`); every other property is
dropped, including nested objects, account IDs as properties, content, and credentials. Unknown
error kinds become `other`. This is enforced in
`app/src/lib/telemetry/posthog.ts` and tested in
`app/src/lib/telemetry/posthog.test.ts`.

## Retention and deletion notes

Desktop code does **not** keep a separate local archive of product analytics
payloads. The local persistence surface is only the disclosure/toggle settings plus
the optional stream's local distinct ID.

Operational notes:

- Clearing app settings/app storage can reset the disclosure state and optional
  stream's distinct ID.
- Desktop builds read `TAURI_POSTHOG_API_KEY` (a public project ingestion token)
  and `TAURI_POSTHOG_HOST` from their environment, with a compiled fallback for
  release bundles. `pnpm dev` reads ignored `app/.env`; build workflows read
  `POSTHOG_PUBLIC_PROJECT_TOKEN` and `POSTHOG_PUBLIC_HOST` GitHub variables.
  Only the US/EU PostHog ingestion hosts are allowed by transport and CSP.
- Local development and public release builds, including beta releases, currently
  use the same **Kolboo Test** PostHog project. This is an explicit
  operational choice because the current plan permits only one project in the
  organization. New events include a fixed `environment` property
  (`development`, `beta`, or `production`); filter on this property for
  release-only reports. Historical events without it remain mixed and must not
  be used as production-only metrics. Build variables must contain this existing project's
  public ingestion token and US host, not a newly generated credential. When a
  separate production project becomes available, replace the release variables
  and update this document before publishing with it.
- An absent token or host disables sending; non-2xx capture responses report a
  status-only warning. HTTP success is not proof of downstream ingestion, which
  should be checked in the PostHog project before a release.
- The desktop sends single events to PostHog's `/i/v0/e/` ingestion endpoint,
  with the required `distinct_id` at the top level and person-profile processing
  disabled in properties.
- Linux beta and Windows release publishing now fail early if the public
  ingestion token or an approved US/EU host is missing from repository variables.
  Ordinary development builds can still run without PostHog configured.
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
- treat the optional random installation ID as an analytics identifier, not
  proof of user identity; an opted-in account ID is a distinct, explicit path
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
