# Sentry Integration (Kolboo)

Last updated: 2026-02-13

This document captures current Sentry research notes, implementation decisions, and operating guidance for the `kolboo` OSS desktop app.

## Goals

- Keep telemetry privacy-safe by default.
- Capture actionable reliability failures.
- Keep local/dev workflows simple (DSN-gated, opt-in by env).
- Ensure the app behaves normally when Sentry is unavailable.

## Current scope (implemented)

Current integration includes **frontend Sentry** for desktop UI surfaces:

- `main`
- `overlay`
- `overlay_hover`
- `quick_ask`

Initialization is DSN-gated and implemented in:

- `app/src/lib/telemetry/sentry.ts`
- Entry points:
  - `app/src/main.tsx`
  - `app/src/overlay-main.tsx`
  - `app/src/overlay-hover-main.tsx`
  - `app/src/quick-ask-main.tsx`

License/account flows route capture via:

- `app/src/lib/tauri/license.ts`

## Environment variables

Configured via Vite env:

- `VITE_SENTRY_DSN`
- `VITE_SENTRY_ENV`
- `VITE_APP_VERSION`

Local placeholders live in:

- `app/.env`

Behavior:

- If `VITE_SENTRY_DSN` is empty/missing, Sentry does not initialize.
- App behavior must remain unchanged with Sentry disabled/unavailable.

## Privacy and redaction rules (required)

Never include user content or secrets in telemetry payloads.

Do **not** capture:

- transcript/dictation text
- prompts/model outputs
- OCR text/images
- API keys/tokens/secrets/cookies

Current protections:

- redaction helper in `app/src/lib/telemetry/sentry.ts`
- event sanitation in `beforeSend`
- network breadcrumbs filtered (`xhr`/`fetch`)
- license telemetry context redaction in both TS and Rust helper paths

## Testing and validation notes

Primary checks used for this rollout:

- `pnpm -C app lint`
- `pnpm -C app typecheck`
- `pnpm -C app test`
- `pnpm -C app cargo:test`
- `pnpm -C app check:ci` (final gate)

Deterministic redaction tests added in:

- `app/src/lib/tauri/license.test.ts`
- `app/src-tauri/src/licensing.rs` (`telemetry_context_redacts_sensitive_fields`)

Manual smoke test (with DSN enabled):

1. Trigger a test frontend exception.
2. Verify event appears in Sentry with expected `environment` and surface tags.
3. Inspect event payload to confirm no sensitive/user-content leakage.

## Project setup guidance (Sentry account)

For temporary pre-prod setup:

- Start with a single Sentry project for desktop app telemetry.
- Platform selection can be React/JavaScript because current integration uses `@sentry/react`.
- Use a dedicated environment label (example: `preprod-personal`).

## Release/dist guidance

Current integration sets environment/release from Vite env variables. As we harden releases, keep runtime values aligned with CI artifact uploads.

Recommended pattern:

- release: `kolboo@<version>`
- dist: `<platform>-<arch>-<build_or_sha>`

## Backend Sentry research direction

This repo does not yet initialize Sentry in Rust/Tauri backend runtime paths.

If/when backend Sentry is added in `kolboo`, keep these decisions:

- use DSN-gated initialization (same safety posture as frontend)
- capture reliability failures only (no content telemetry)
- keep strict redaction and whitelist-style metadata
- preserve normal app behavior when Sentry is unavailable

Candidate backend capture points:

- Tauri command failures with sanitized context
- startup/bootstrap failures
- pipeline/state-transition failures with enum/status metadata only
- panic capture with sanitized payloads

## Non-goals for now

- Session Replay
- Profiling
- Broad performance tracing

These can be evaluated in later phases with explicit privacy and sampling controls.
