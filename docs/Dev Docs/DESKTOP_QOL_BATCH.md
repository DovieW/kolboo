# Desktop quality-of-life batch — September 2026

This batch was selected from the product backlog for high value without new
credentials, paid requests, production changes or release publication.

## Delivered scope

- Responsive, accessible sidebar; bounded desktop content instead of edge-to-edge
  Settings/Logs; coordinated sticky Settings title/profile/tabs; concise Home/Usage
  headings and a direct Transcribe file destination.
- Logs separate ordinary request history from optional System Events. Clearing
  logs and exporting full content require an explicit confirmation.
- API-key replacement fields save on Enter/blur, report status/failures and require
  confirmation for removal. Existing secrets are not fetched into the webview just
  to display the settings page. Blank drafts do not delete keys. Profile locks also
  block keyboard access rather than relying only on pointer styling.
- Account copy distinguishes **access level** from the actual provider route;
  having managed access does not imply every request uses managed services.
- Home Escape/Cancel retains recoverable audio; F3 cancellation keeps its existing
  non-persistent semantics. Home Hot Mic shutdown drains and syncs queued audio.
- Local file transcription reuses the complete-recording/History pipeline with
  explicit upload, independent Meeting selection, cancellation and resumable retry.
  Initial formats are WAV, MP3, FLAC and ADTS AAC. M4A, Ogg and AIFF remain
  unsupported pending decoder/container hardening; no custom media parser was added.

## Deliberately separate

- A global managed-off preference needs one native routing gate across dictation,
  meetings and rewriting, with settings migration and tests. No UI-only switch was
  added because it would falsely imply all routes were covered.
- New providers/models require verified current upstream contracts and pricing;
  a backlog name or announcement is not sufficient to enable a paid route.
- File-manager integration, a dedicated drop overlay and new global shortcuts need
  platform lifecycle/permission acceptance beyond the new in-app import page.
- Subscription changes, account moves, repository-history rewriting, branding,
  model training and enterprise identity remain separate authorized work.
- Coverage stays risk-based. More tests do not constitute a claim of 100% coverage
  or verified physical audio/platform behavior.

## Acceptance

Automated tests cover local decoding/source preservation, interrupted import cleanup,
recovery ownership, cancellation, upload checkpoints, UI selection versus explicit
upload, retry reuse, navigation, key editing and destructive-action confirmation.
Use synthetic audio and mocked providers; do not require credentials or paid calls.

Manual screenshots and native checks are still required for narrow/wide layouts,
file picker and native drop behavior, actual audio playback, microphone/Hot Mic
permissions, Escape capture, and platform packaging. These are not automated UI or
cross-platform release claims. Managed production expansion remains separately
gated by server-side safety validation and operational sign-off. This desktop
batch does not implement or claim a cloud-spending cap.

## Validation evidence — 2026-09-07

- Frontend suite: **706 passed, 57 skipped**. The final supported-format filter
  and file-transcription component were also rerun together: **14 passed**.
- Native suite: **855 passed, 12 ignored**, including import fixtures, recovery
  ownership and generated-schema equality checks.
- TypeScript checking, frontend lint, Knip, all-target Clippy, Rust formatting,
  renderer build and the native `local-whisper` feature check passed. Existing
  lint/platform warnings, two Knip configuration hints and the renderer's large
  chunk warning remain; this is not a warning-free build claim. Sentry upload was
  explicitly disabled for the renderer build.
- Contracts regenerated: 63 JSON schemas, 51 generated TypeScript type blocks
  and 30 Tauri event names. Generated JSON schemas follow the repository's
  existing ignored-build-artifact convention; their equality is tested natively.
- An initial frontend run overlapping native linking hit resource-contention
  timeouts. The complete suite passed after serialization, without relaxing test
  timeouts.

These checks used local/synthetic fixtures and mocked providers, not paid API
requests. No native GUI, microphone, four-hour physical recording, Windows/macOS
build, release package or live deployment was verified in this batch. `cargo-audit`
and `cargo-machete` are not installed here; a complete security/dependency gate was
not run. The strict Rust dead-code gate is Windows-only and does not run on this
Linux host. See [the file-backed long-audio follow-up](../Refactors/4_FILE_BACKED_LONG_AUDIO.md)
for remaining whole-recording memory and forced-task-drop limitations.

## Recorder validation follow-up — 2026-09-07

- Recording controls wait for persisted preferences before enabling Record or
  mode/model changes. Loading defaults cannot overwrite a saved meeting model;
  failed preference reads offer a retry without changing the saved selection.
- The meeting model picker distinguishes loading, failed sources and genuinely
  empty catalogs. Failed sources can be retried independently, and available
  BYOK/local choices remain usable when the managed catalog fails. There is no
  bundled managed-model fallback or automatic replacement of a saved model.
- Model-save failures stay inside the model dialog instead of opening another
  dialog on top. Pending saves prevent dismissal and duplicate submissions.
- Native runtime configuration always includes the package version when no
  explicit version override is supplied. An intentionally cloud-free launch is
  no longer mistaken for an unavailable IPC response; telemetry remains disabled
  without a DSN.
- Follow-up checks: **52 focused frontend tests** and **2 native configuration
  tests** passed, alongside TypeScript, focused Biome lint, Rust formatting and
  diff whitespace checks. The full suites above were not repeated for this patch.
- Captured the actual wide-screen Settings/Appearance window, then restarted the
  native development app and verified Home at 1280 × 720 with its compact idle
  recording bar. The frontend returned HTTP 200, and the rebuilt startup no
  longer reported runtime-config fallback retries. Screenshots remain temporary
  local artifacts. The file-import page, recorder popup, audio devices and other
  narrow-screen layouts still need visual or hardware acceptance; this check
  does not claim those paths were exercised.

The dev session remains running. After onboarding, native restarts intentionally
start in the tray; launching the same executable again reveals the existing
window through the single-instance handler. No credentials were provisioned and
no paid API call, release or production deployment was performed.

## File-transcription defaults follow-up — 2026-09-07

The retained import page previously copied recorder preferences while hidden,
before any file was chosen. It now follows current defaults until file selection
or an explicit options edit, then pins that draft across navigation. If preferences
arrive after file selection, the first loaded options are pinned as well. This
does not write recorder defaults or alter recovery's persisted options.

A single loading status explains unavailable controls while preferences or recorder
state load. A regression test reproduced the stale-mode behavior before the fix;
the four focused frontend files now pass **38 tests**, including late preference
loading, explicit-choice preservation and unchanged import/recovery behavior.
