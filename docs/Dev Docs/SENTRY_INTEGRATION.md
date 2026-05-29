# Sentry Integration (Kolboo)

Last updated: 2026-05-29

This document captures current Sentry research notes, implementation decisions, and operating guidance for the `kolboo` OSS desktop app.

Canonical-plan note:

- The cross-repo Sentry plan now lives in
  `kol-software/plans/KOLBOO_SENTRY_END_TO_END_PLAN.md`.
- This file is intentionally desktop-specific. Keep it focused on `kolboo`
  implementation details, validation notes, and operating guidance rather than
  turning it into the master multi-repo plan.

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
- `app/src/lib/bootstrap/renderRoot.tsx` (React 19 root error hooks)
- Entry points:
  - `app/src/main.tsx`
  - `app/src/overlay-main.tsx`
  - `app/src/overlay-hover-main.tsx`
  - `app/src/quick-ask-main.tsx`

The app intentionally keeps its custom fallback UI / panic overlays. React 19
root error hooks are wired in so Sentry still observes uncaught, caught, and
recoverable render failures without replacing those UX paths.

Current integration also includes **backend Sentry** for the Tauri runtime:

- DSN-gated init in `app/src-tauri/src/sentry_init.rs`
- startup wiring from `app/src-tauri/src/lib.rs`
- deterministic backend smoke command in `app/src-tauri/src/commands/logs.rs`

License/account flows route capture via:

- `app/src/lib/tauri/license.ts`

## Environment variables

Configured via backend runtime env and surfaced to the frontend through
`get_runtime_config`:

- `TAURI_SENTRY_DSN`
- `TAURI_SENTRY_ENV`
- `TAURI_SENTRY_RELEASE`

Local placeholders/defaults live in:

- `app/.env`
- repo-local CLI helper defaults in `.sentryclirc`

Behavior:

- If `TAURI_SENTRY_DSN` is empty/missing, frontend and backend Sentry stay disabled.
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
- React 19 root hooks capture renderer failures without adding replay/autocapture
- license telemetry context redaction in both TS and Rust helper paths
- backend `before_send` scrubbing in `app/src-tauri/src/sentry_init.rs`
  - drops `user`, `request`, and `server_name`
  - redacts sensitive event messages / exception values / tags
  - recursively redacts sensitive `extra` payloads and breadcrumb data by key or content markers

## Testing and validation notes

Primary checks used for this rollout:

- `pnpm -C app lint`
- `pnpm -C app typecheck`
- `pnpm -C app test`
- `pnpm -C app cargo:test`
- `pnpm -C app check:ci` (final gate)

Deterministic redaction tests added in:

- `app/src/lib/telemetry/sentry.test.ts`
- `app/src/lib/tauri/license.test.ts`
- `app/src-tauri/src/licensing.rs` (`telemetry_context_redacts_sensitive_fields`)
- `app/src-tauri/src/sentry_init.rs`

Manual smoke test (with DSN enabled):

1. Trigger a test frontend exception.
2. Verify event appears in Sentry with expected `environment` and surface tags.
3. Inspect event payload to confirm no sensitive/user-content leakage.
4. Trigger `logsAPI.sentryBackendSmokeTest(...)` and confirm the backend smoke
  event arrives with `runtime=tauri-backend`, the expected surface tag, and no
  request/user payloads.

## Project setup guidance (Sentry account)

Current org topology is split by surface family and environment:

- `kolboo-public-dev`
- `kolboo-public-prod`
- `kolboo-private-dev`
- `kolboo-private-prod`

Repo-local CLI defaults in `kolboo/.sentryclirc` point local commands at
`kolboo-public-dev`.

The public project currently uses a React/JavaScript platform choice because the
desktop app's user-facing surfaces are React entrypoints. That platform choice
does **not** prevent the shared DSN from also receiving Tauri backend events; the
DSN routes events to the project, while the selected project platform mainly
drives setup guidance and defaults.

## Release/dist guidance

Current integration sets environment/release from Vite env variables. As we harden releases, keep runtime values aligned with CI artifact uploads.

Recommended pattern:

- release: `kolboo@<version>`
- dist: `<platform>-<arch>-<build_or_sha>`

## Backend next-capture direction

Backend Sentry is now initialized for the Tauri runtime, but the launch-hardening
work should still stay conservative:

- keep DSN-gated initialization (same safety posture as frontend)
- capture reliability failures only (no content telemetry)
- keep strict redaction and whitelist-style metadata
- preserve normal app behavior when Sentry is unavailable

Candidate future backend capture points:

- Tauri command failures with sanitized context
- startup/bootstrap failures
- pipeline/state-transition failures with enum/status metadata only
- panic capture with sanitized payloads

## Non-goals for now

- Session Replay
- Profiling
- Broad performance tracing

These can be evaluated in later phases with explicit privacy and sampling controls.
