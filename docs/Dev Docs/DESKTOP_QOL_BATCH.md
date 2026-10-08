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

## Startup preferences — 2026-10-07

Settings → UI now has independent global controls for **Start at login** and
**Show window on launch**. The latter defaults off: after onboarding, manual and
login launches remain tray-only unless enabled. The first-run setup window stays
visible regardless of that preference; launching an already-running instance
still brings its window forward. This does not change close-button behavior.

Login startup is enabled on fresh installations via the existing cross-platform
autostart plugin. Before settings are seeded, the empty-store check distinguishes
new installations from upgrades. Existing OS startup registrations and opt-outs
are preserved and are never reenabled at every launch. CLI invocations never
register login startup. Registration failure does not prevent opening Kolboo;
the UI continues to read and verify actual OS state and permits retrying there.
The OS registration remains machine-local and is not synced or backed up.

The window visibility preference is a non-secret global setting in
`settings.json`, with normal settings normalization, patch persistence and query
invalidation. It applies on the next launch, not by unexpectedly showing/hiding
the current window or refreshing overlays/pipeline configuration. It follows
other general UI preferences through explicit settings backup/sync, not profiles.
The isolated Linux native fixture verifies real enable/disable registration,
upgrade opt-out preservation, and production window startup with onboarding
already completed and show-on-launch enabled. Windows/macOS use the same plugin
and policy but still require native acceptance; Linux results do not certify them.

Validation: the full local `check:ci` checkpoint passed (933 frontend tests and
980 Rust tests, with the documented skips), as did the focused settings-sync
test and the isolated native fixture/production startup. Changed executable code
has 100% patch coverage without new exceptions. The optional native-test tools
were extracted into a temporary directory on the development host; no system
installation or namespace security setting was changed.

IdeaPad acceptance: installed the service-enabled development Debian package
through the existing user-local executable, preserving data and a rollback copy.
Verified the running executable against the extracted package, healthy frontend
readiness, and both rendered UI controls through AT-SPI. Enabled Show window on
launch in the actual UI, confirmed persistence, restarted the process and
verified the main window was showing. Restored that preference to off afterward;
the existing enabled login registration was unchanged. No actual logout/reboot
or Windows/macOS desktop acceptance was performed.

## 0.3.1 release preparation — 2026-10-01

Package, Cargo and Tauri versions are aligned at 0.3.1, with user-facing notes in
`changelog.d/0.3.1.md`. The release workflow now includes Linux alongside Windows
and experimental universal macOS, and prepares a draft. Exact-package acceptance
and explicit publication still follow successful builds; the version bump is not
a publication claim. Windows/macOS native acceptance remains maintainer-deferred.

Fresh local `check:ci` and `coverage:patch` passed in about 2m06s each: 901 frontend
tests, 951 Rust tests, plus isolated native-window/startup acceptance. The patch
gate covers all 405 non-exempt changed executable lines, with the 247 previously
approved native/race line exceptions across 13 reviewed source snapshots still
reported separately. The JavaScript test toolchain was updated to Vitest 4.1.11
to resolve its development-server file-read advisory; the JavaScript audit is
clean. The combined JavaScript/Rust audit passed (Rust reports eight existing
allowed warnings), as did cargo-machete and actionlint on the changed workflows.
This does not establish release-artifact acceptance or native behavior on other
operating systems.

The investigated IdeaPad termination on September 30 was SIGKILL, with no logged
sender, kernel OOM event or Rust/WebKit crash report found. The app subsequently
restarted and completed transcriptions. No general crash fix is claimed in the
release notes.

## Persistent desktop input — 2026-09-30

Wayland paste now uses an app-identified, keyboard-only RemoteDesktop portal
session rather than opening another XWayland synthetic-input connection for
each action. The desktop-issued restoration token remains backend-only in OS
secure storage, is rotated after restoration, and is never synced or exposed
to renderer secret commands. Normal dictation, Paste Last, Retry Last, explicit
output and selection-copy share the grant. Optional media control reuses an
approved session without requesting permission during recording.

Denied/unsupported setup falls back to the clipboard and does not ask again
for every recording in the same launch. Unsupported desktop stacks are detected
by capabilities, not distro names. Revocation, missing persistence or a locked
keyring can still require approval again. Linux packages include matching
`com.kolboo.app.desktop` identity metadata without an extra visible launcher;
portable development installs still require host desktop integration.

Clipboard preparation waits for approval. Output leases reject delayed pastes
after a newer pipeline operation begins; failed key delivery cleans up without
automatically replaying a possibly delivered chord. Linux/Windows async output
runs on a worker; macOS native input stays on the main thread and opens the
Accessibility prompt at most once per launch while still checking OS approval.
Windows UIA and X11 native output retain their existing paths.

The maintainer approved narrow native-boundary coverage exceptions and deferred
Windows/macOS native acceptance for the next release. The refreshed build was
installed and restarted on the online IdeaPad on September 30. Its KDE Wayland
desktop advertises RemoteDesktop portal version 2; the running executable hash
matches the extracted package, the overlay reported frontend readiness, and the
main window exposed its rendered controls through AT-SPI after being focused.
Actual KDE approval, repeated paste and grant restoration after app restart
remain pending user interaction. No release publication was performed.
Linux protocol tests do not certify another OS or
every compositor. See `LINUX_DEVELOPMENT.md` for the native acceptance checklist.

The user-local binary was replaced atomically, without sudo or changes to the
system package, recordings, credentials or settings. The stale user-local hidden
portal identity referenced an old repository executable; it was replaced with
the package's validated `com.kolboo.app.desktop`. The visible launcher and login
autostart entry were preserved. Rollback copies of the executable and desktop
entries are in `~/.cache/kolboo-dev/desktop-input-TmNyrfwv/`. The running app is
supervised by the transient user unit `kolboo-dev-20260930-input.service`; this
unit does not replace or add login autostart configuration.

Validation on this Linux development machine: `setup:check` and `check:ci`
passed; 898 frontend tests and 943 Rust tests passed (57 frontend skipped,
13 Rust ignored). The patch gate reports 260 covered changed executable lines
and 248 explicitly exempt native/defensive lines across 13 reviewed snapshots.
The refreshed debug-profile Debian package built in 50.3 seconds; its contents
include both the visible Kolboo launcher and hidden matching portal identity.
It is an installable development package, not a published release artifact.

## Main-window sizing and persistence — 2026-09-30

The IdeaPad's main window was 1280 physical pixels wide while XSettings exposed
168 DPI (175% content scaling). The main window did not compensate for this
fractional XWayland mismatch and had no persisted size/maximized preference.

The fix restores logical content dimensions before showing either
the startup window or a window recreated from the tray. It bounds sizes and
minimum sizes to the current work area, saves normal dimensions separately from
maximized state, and ignores minimized/fullscreen/transient-zero measurements.
Preferences use a separate machine-local `main-window.json` store with debounced
resize writes and a close-time flush; they do not enter settings sync or backups.
Windows and macOS use their native scale factor without applying Linux DPI again.

Eight deterministic regression tests cover sizing, work-area fallback, real
temporary-store save/reload and failed-save recovery. An opt-in Linux integration
test uses real GTK/Tauri windows in isolated Xvfb/Openbox to exercise resizing,
maximizing, production tray recreation and failed native/store operations. Offline
production startup/close is also instrumented. Linux Rust coverage merges those
profiles into the regular gate; no new native exception was added for this fix.
Duplicate Rust build identities are merged without collapsing source locations,
generic types or closure indices; focused coverage-helper tests protect that rule.

Validation: 901 frontend tests, 951 Rust tests and the real-window test passed;
type checking, focused script lint, Rust formatting and 100% non-exempt patch
coverage passed. Existing previously approved native exceptions remain; this is
not global coverage. The fast installable Linux package built in 44.8 seconds.

IdeaPad acceptance: atomically updated the user-local executable, with a rollback
copy at `/home/dovie/.cache/kolboo-dev/window-size-NPQviLxn/kolboo.previous`.
At 175% content scaling, normal dimensions are now 2240×1400 physical pixels
(1280×800 logical). A resize to 2100×1313 survived close/recreate; maximized state
also survived close/recreate and a full process restart. Reset the normal size
to the corrected default and left the app running maximized, matching its
pre-update state. The running binary hash matched the extracted package and the
rendered web document was verified through accessibility. Windows/macOS scaling
policies are unit-tested; native acceptance there remains deferred, not certified.

## Start at login — 2026-09-29

Settings → UI now includes **Start at login**, a machine-global switch in the
Default profile, independent of the Close button setting, and does not record
or upload anything at startup. Initially opt-in, it now defaults on for fresh
installations (see the October 7 startup preferences above); upgrades preserve
the existing OS choice. Completed-setup launches still default to the tray,
with the new Show window on launch preference available to change that.

The official Tauri autostart plugin manages the current executable through XDG
autostart on Linux, the Windows login registry, or a macOS LaunchAgent. Only the
main window has autostart permissions. The registration is read on opening the
tab and after writes; failures do not optimistically flip the saved state. This
machine-local choice is deliberately not stored in settings.json, synced to an
account, exported with profiles, or overridden per program. Moving an AppImage
or installing the executable at a different path requires disabling/re-enabling
the setting from the new location.

Tests exercise the real JavaScript plugin/IPC boundary with deterministic OS
responses: loading, existing registration, enable/disable/readback, failed reads
and writes, retry, duplicate-write prevention, and profile-scope restrictions.
Native acceptance remains platform-specific; a Linux check does not certify
Windows/macOS login or replace an actual logout/reboot test.

IdeaPad acceptance: the real main-window switch was activated via AT-SPI
accessibility actions. Enable created `~/.config/autostart/Kolboo.desktop` with
the stable `~/.local/bin/kolboo` executable; disable removed it; enable restored
it. The switch remained enabled after restarting the actual app. The desktop
entry passed `desktop-file-validate`. The maintainer approved
an exact one-line native plugin-registration coverage exception; all added UI
lines, branches and functions are covered. No account/session data was changed.

Validation: 898 frontend tests and 930 Rust tests passed (57 frontend skipped,
13 Rust ignored); focused startup tests, typecheck, lint, Knip, Rust formatting,
schema generation and the patch gate passed. The exact one native-wiring
exception is printed by the gate. The laptop uses its existing stable
`~/.local/bin/kolboo` launcher, updated atomically from the built Debian package
without sudo. The system package is untouched; future development deployments
must update this user-local binary, or deliberately restore the saved launcher
symlink before switching back to system-package installations. Login autostart
is enabled on this laptop only. No real logout/reboot was performed.

### Ubuntu font-cache incident

On the IdeaPad, WebKit renderers were stuck in font lookup and `fc-match`
incorrectly selected `KaTeX_AMS-Regular.woff` for ordinary UI fonts. The user
font cache contained cache-9/10/11 symlinks pointing to incompatible cache-12
files. Repair preserved the old cache, rejected only KaTeX `.woff`/`.woff2`
system-font matches (not its TTF fonts), and rebuilt the user cache using the
installed system Fontconfig. Normal font matching and frontend-ready events
returned. No fonts were removed, system packages changed, or rendering/sandbox
security disabled. This is a laptop-specific OS workaround, not a Kolboo
startup-time font-cache mutation.

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

## History cleanup follow-up — 2026-09-07

- Removed the sidebar's active inset stripe, routine Saved badges, repeated expanded
  previews and duplicate reader title. Title editing lives in Edit mode.
- Playback failures stay inline rather than also opening an error toast. Closing
  full view cancels playback while preserving preparation for the inline player.
- Copy reads the active or retained document draft before persisted text, so a
  failed save does not silently copy an older correction.
- Deletion owns its lookup and mutation synchronously: repeated clicks cannot
  replace a shared-recording confirmation's target or start duplicate deletions.
  Existing shared-recording deletion scopes and failure rollback remain intact.
- The five focused history/recording-control test files pass 32 tests. These use
  disposable fixtures; no user recordings are deleted by the checks.
- Restarted `pnpm dev` and captured the native Home window at 1280×720. Native
  pointer injection did not activate controls on this Wayland desktop, so this
  is a Home visual check, not a manual accordion/modal/deletion acceptance run.

## Ad-hoc network settings follow-up — 2026-09-08

Quick Ask, Quick Replace, History analysis and Settings/Prompt Lab LLM requests
now pass the current pipeline proxy/TLS settings into their HTTP clients, for
built-in and custom providers alike. Custom endpoints retain their no-redirect
policy. Timeout and structured-output choices are unchanged. No IPC contract or
stored setting changed.

Regression checks cover invalid manual proxies for every provider and actual
local mocked proxy/bypass/No proxy requests through both ad-hoc constructors.
The full native library suite passes: 864 passed, 12 ignored; no paid calls.
