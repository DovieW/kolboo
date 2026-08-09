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

Packaged desktop delivery also now depends on the Tauri webview CSP allowing
the Sentry ingest origin. The 2026-05-29 packaged rehearsal proved that a
restrictive packaged `connect-src` can surface as frontend
`transport send failed Failed to fetch`, even when DSN/runtime-config/init/capture
all succeed locally.

License/account flows route capture via:

- `app/src/lib/tauri/license.ts`

## Environment variables

Configured via backend runtime env and surfaced to the frontend through
`get_runtime_config`:

- `TAURI_SENTRY_DSN`
- `TAURI_SENTRY_ENV`
- `TAURI_SENTRY_RELEASE`
- `TAURI_SENTRY_SMOKE` (manual packaged-app rehearsal only)

For browser-only frontend verification via `pnpm -C app dev:vite`, the desktop
frontend also accepts optional Vite env fallbacks when the Tauri runtime bridge
is unavailable:

- `VITE_SENTRY_DSN`
- `VITE_SENTRY_ENV`
- `VITE_SENTRY_RELEASE`

Local placeholders/defaults live in:

- `app/.env`
- repo-local CLI helper defaults in `.sentryclirc`

Behavior:

- If `TAURI_SENTRY_DSN` is empty/missing, frontend and backend Sentry stay disabled.
- If `TAURI_SENTRY_SMOKE=1`, the packaged desktop app can emit the explicit
  non-production frontend smoke event during a manual rehearsal launch.
- If the frontend is running outside Tauri (for example `pnpm -C app dev:vite`),
  the browser surface may opt in via `VITE_SENTRY_*` for smoke verification.
- Frontend runtime-config loading now treats real Tauri sessions differently from
  browser-only sessions: it retries the `get_runtime_config` bridge for longer in
  packaged/desktop startup races, but falls back quickly outside Tauri so browser
  smoke runs do not stall.
- If that startup bridge still fails after the retry window, the frontend no
  longer permanently memoizes the all-default fallback; later callers can retry
  runtime-config loading instead of getting stuck with an empty config for the
  rest of the session.
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
- frontend Sentry init/smoke flow now also emits sanitized breadcrumbs into the
  desktop rolling log via the frontend log bridge (`scope=sentry`), so packaged
  rehearsals can distinguish “no runtime config / no DSN” from “smoke capture
  attempted and flushed” without relying on DevTools

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

Desktop frontend now also supports an explicit non-production browser smoke
trigger via `?kolboo_sentry_smoke=1` on the `main` surface. This is especially
useful with `pnpm -C app dev:vite`, where `VITE_SENTRY_*` can verify the
browser-side release/tag wiring without launching the full Tauri shell. In that
browser-only smoke mode, the `main` entrypoint intentionally stops before the
normal desktop boot/render path when `core.isTauri === false` so expected
missing-bridge noise does not create extra fake issues. The smoke helper also
waits for `Sentry.flush(2000)` before returning so short-lived/headless
verification runs do not drop the event on page exit.

## Verified non-production frontend smoke evidence

Verified on 2026-05-29 via `pnpm -C app dev:vite` + headless Edge:

- project: `kolboo-public-dev`
- release: `kolboo@preview.localsmoke.2026-05-29-c`
- smoke URL: `http://127.0.0.1:4191/?kolboo_sentry_smoke=1`
- issue: `KOLBOO-PUBLIC-DEV-E`
- issue link: <https://dov-weinstock.sentry.io/issues/7513048876/>
- latest verified event id: `11df1e65f17f4a52bbfd0c16eab28c22`
- durable evidence note: `../../kol-software/plans/KOLBOO_SENTRY_REHEARSAL_EVIDENCE_2026-05-29.md`

What this proves:

- the desktop frontend can emit a real non-production browser smoke event into
  `kolboo-public-dev`
- the smoke tags (`surface=main`, `action=smoke_test`, `smoke_test=true`,
  `smoke_trigger=query-param`) and release metadata are searchable in Sentry
- the browser-side `VITE_SENTRY_*` fallback is sufficient for frontend-only
  verification when the full Tauri runtime is intentionally absent

Known limitation of this evidence path:

- even after short-circuiting the main render path, browser-only `dev:vite`
  verification can still produce some expected Tauri-bridge TypeErrors from
  modules that assume the desktop runtime exists. Treat those as verification
  noise, not launch evidence. The smoke issue above is the durable proof item.

## Verified uploaded-release/source-map artifact proof

Verified on 2026-05-29 via a rebuilt static desktop artifact + manual Sentry
release upload flow:

- project: `kolboo-public-dev`
- release: `kolboo@artifactproof.2026-05-29-c`
- local artifact URL: `http://127.0.0.1:4301/index.html?kolboo_sentry_smoke=1`
- issue: `KOLBOO-PUBLIC-DEV-F`
- issue link: <https://dov-weinstock.sentry.io/issues/7513688933/>
- latest verified event id: `c392b8d3b813484bad9a9882cd8b7529`
- durable evidence note: `../../kol-software/plans/KOLBOO_SENTRY_REHEARSAL_EVIDENCE_2026-05-29.md`
- verified mapped locations:
  - `../../src/lib/telemetry/sentry.ts:230:7`
  - `../../src/lib/telemetry/sentry.ts:218:3`
  - `../../src/main.tsx:8:26`

What this proves:

- the uploaded release `kolboo@artifactproof.2026-05-29-c` received a real
  browser event from the built static bundle, not only the dev-server path
- the uploaded source maps resolve the minified bundle back to readable source
  locations in `src/lib/telemetry/sentry.ts` and `src/main.tsx`
- the desktop frontend release/source-map artifact path is now proven locally
  for the public-dev project

Known limitation of this artifact-proof path:

- browser-only artifact verification still produces some expected Tauri-bridge
  noise (`Cannot read properties of undefined (reading 'invoke')`) because the
  full desktop runtime is absent; treat those as verification noise, not the
  proof item

What this does **not** prove yet:

- final public-prod release-cut evidence tied to the exact production desktop
  delivery path

For the packaged Windows follow-up path, use:

- `docs/Dev Docs/SENTRY_WINDOWS_PACKAGE_REHEARSAL.md`

## Verified packaged Windows delivery proof

Verified on 2026-05-29 in a real packaged Windows Tauri install after patching
`app/src-tauri/tauri.conf.json` so packaged `connect-src` allows Sentry ingest:

- project: `kolboo-public-dev`
- release: `kolboo@packagerehearsal.2026-05-29-e`
- issue: `KOLBOO-PUBLIC-DEV-H`
- issue link: <https://dov-weinstock.sentry.io/issues/7513912680/>
- latest verified event id: `ad3b36c90018461eb6117e75cde83175`
- event link: <https://dov-weinstock.sentry.io/issues/7513912680/events/ad3b36c90018461eb6117e75cde83175/>
- installed executable:
  `C:\Users\Dovie\AppData\Local\Programs\Kolboo-Packagerehearsal-e\Kolboo.exe`
- durable evidence note:
  `../../kol-software/plans/KOLBOO_SENTRY_REHEARSAL_EVIDENCE_2026-05-29.md`
- decisive packaged log proof:
  - `[ui:sentry] initialized surface=main env=preview release=kolboo@packagerehearsal.2026-05-29-e smoke_requested=true`
  - `[ui:sentry] smoke capture surface=main trigger=runtime-env release=kolboo@packagerehearsal.2026-05-29-e`
  - `[ui:sentry] transport send status=200 rate_limits=none retry_after=none`
  - `[ui:sentry] smoke flushed surface=main trigger=runtime-env release=kolboo@packagerehearsal.2026-05-29-e`
  - secondary surfaces `overlay`, `overlay_hover`, and `quick_ask` also logged
    `transport send status=200`

What this proves:

- the real packaged Windows/Tauri shell now delivers frontend Sentry envelopes
  successfully
- the packaged smoke event is also visible upstream in Sentry with the expected
  `release`, `environment=preview`, `surface=main`, and `smoke_trigger=runtime-env`
- the earlier packaged failure (`transport send failed Failed to fetch` on
  release `kolboo@packagerehearsal.2026-05-29-d`) was caused by packaged CSP,
  not by DSN resolution, runtime-config loading, or local capture/flush logic

## Verified GitHub-built Windows bundle proof

Verified on 2026-06-07 using the successful Windows workflow build artifact and
its matching `windows-build-evidence` metadata from run `26787316293`:

- project: `kolboo-public-dev`
- release: `kolboo@0.2.4-dev.80dcfa8`
- workflow run: `26787316293`
- workflow link: <https://github.com/DovieW/kolboo/actions/runs/26787316293>
- evidence artifact: `windows-build-evidence`
- bundle artifact: `kolboo-windows-bundles`
- launch path used for rehearsal:
  `C:\Users\Dovie\AppData\Local\Temp\kolboo-windows-bundles-26787316293\kolboo.exe`
- note: this used the loose bundled exe fallback from the GitHub artifact
  instead of an installed MSI/NSIS path; the installed-path proof remains the
  2026-05-29 packaged rehearsal above
- issue: `KOLBOO-PUBLIC-DEV-F`
- issue link: <https://dov-weinstock.sentry.io/issues/7513688933/>
- latest verified event id: `7d02628843724cf3b0cebee232e1cc9a`
- event link:
  <https://dov-weinstock.sentry.io/issues/7513688933/events/7d02628843724cf3b0cebee232e1cc9a/>
- verified mapped locations:
  - `../../src/lib/telemetry/sentry.ts:399:4`
  - `../../src/lib/telemetry/sentry.ts:383:2`
  - `../../src/main.tsx:8:25`
- decisive local log proof:
  - `Backend Sentry initialized`
  - `[ui:sentry] initialized surface=main env=development release=kolboo@0.2.4-dev.80dcfa8 smoke_requested=true`
  - `[ui:sentry] smoke capture surface=main trigger=runtime-env release=kolboo@0.2.4-dev.80dcfa8`
  - `[ui:sentry] transport send status=200 rate_limits=none retry_after=none`
  - `[ui:sentry] smoke flushed surface=main trigger=runtime-env release=kolboo@0.2.4-dev.80dcfa8`

What this proves:

- the GitHub-produced Windows bundle artifact and the `windows-build-evidence`
  release metadata now line up with a real upstream Sentry event in
  `kolboo-public-dev`
- the public-dev desktop browser/source-map path is no longer only a local
  rebuild proof; a real GitHub workflow artifact now resolves back to readable
  source frames in `src/lib/telemetry/sentry.ts` and `src/main.tsx`
- the remaining desktop Sentry gap is no longer "can GitHub-produced Windows
  artifacts prove the release/source-map path?"; it is only whether we still
  want an explicit public-prod/final-release rehearsal in addition to the
  existing public-dev evidence

## Build-time source maps

The desktop Vite build now wires `@sentry/vite-plugin` for browser-source-map
upload when build-time auth is configured.

Build-time env contract:

- `SENTRY_AUTH_TOKEN` — required to enable upload
- `SENTRY_ORG` — defaults to `dov-weinstock`
- `SENTRY_PROJECT` — optional explicit override; otherwise the Vite build
  derives `kolboo-public-dev` vs `kolboo-public-prod` from
  `TAURI_SENTRY_ENV`
- `SENTRY_RELEASE` — optional explicit override; otherwise the build uses
  `TAURI_SENTRY_RELEASE` and falls back to a deterministic `kolboo@...` value

Behavior:

- without `SENTRY_AUTH_TOKEN`, the plugin stays disabled and release builds keep
  behaving normally
- with `SENTRY_AUTH_TOKEN`, the Vite build emits **hidden** source maps,
  uploads them to Sentry, and deletes the generated `.map` files from `dist`
  after upload completes
- the Windows release workflow now computes release metadata and passes the
  matching public dev/prod project + release name into the build so release
  artifacts and uploaded source maps stay aligned
- the Windows release workflow now also uploads a `windows-build-evidence`
  artifact containing the computed Sentry project/release and the uploaded
  bundle artifact names, so shipped-package rehearsal notes can link back to the
  exact browser-source-map release metadata
- the packaged desktop app now also supports a manual rehearsal-only smoke gate
  via `TAURI_SENTRY_SMOKE=1`, which is the supported trigger for proving the
  Windows installer/bundle path in a real Tauri shell

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
