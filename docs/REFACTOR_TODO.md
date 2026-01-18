# Refactor ideas (out of scope)

This file is a parking lot for larger refactors that came up while working on smaller changes.

## Overlay UI (React)

- **Split `app/src/OverlayApp.tsx` into smaller modules.**

  - Suggested extraction targets:
    - `RecordingControl` (top-level controller)
    - `BackendAudioWave` + rendering helpers
    - `AudioWave` (browser analyser fallback)
    - Hover gating logic (mouse tracking + suppress-on-show)

- **Extract the overlay UI reducer into a dedicated hook.**

  - Move the reducer + action types into something like `app/src/lib/useOverlayUiReducer.ts`.
  - Add a short transition table comment that explains how the UI should behave when:
    - hotkey fires before `pipeline-state-changed`
    - polling returns a stale state
    - recording-only mode hides right after going idle

- **Consider a single “overlay controller” state object.**
  - Right now some state lives in refs, some in `useState`, some in the reducer.
  - A follow-up could consolidate more of this into one predictable state machine, but that’s a larger change.

## Biggest “hot spot” files by size (worth refactoring)

These are the files that are *currently* the largest / most responsibility-dense. They aren’t “bad”, but they’re the most likely to become painful to change.

### Rust backend

- **Split `app/src-tauri/src/lib.rs` (~256KB).**
  - Why: it currently mixes app bootstrap, tray/window behavior, hotkeys, settings seeding/migration, pipeline orchestration, Quick Ask / Quick Replace flow wiring, and lots of event emission.
  - Suggested splits (modules + functions):
    - `bootstrap/*` (plugins, window creation, menu/tray setup)
    - `shortcuts/*` (global shortcut registration + Escape-to-cancel lifecycle)
    - `sessions/*` (record start/stop orchestration; Quick Ask / Quick Replace branches)
    - `settings/defaults.rs` (keep `ensure_default_settings(...)` + migrations close to settings types)
    - `overlay/*` (show/hide/position logic)
  - Acceptance hint: the public Tauri command API stays the same; this is mostly moving code + adding thin wrappers.

- **Split `app/src-tauri/src/pipeline.rs` (~188KB).**
  - Why: it contains unrelated concerns (provider construction/caching, routing logic, state machine + config, embedding cache/persistence, and helper utilities).
  - Suggested splits:
    - `pipeline/state.rs` (state machine + transitions/guards)
    - `pipeline/config.rs` (PipelineConfig defaults + normalization)
    - `pipeline/providers.rs` (STT/LLM provider creation + caching)
    - `pipeline/router/*` (embeddings router + LLM router + diagnostics payload building)
  - Bonus: lots of helper functions here are pure (e.g. path normalization / routing scoring) and can get fast unit tests once extracted.

- **Break up `app/src-tauri/src/commands/text.rs` (~32KB) by responsibility.**
  - Why: it’s doing 3 tricky OS-level jobs in one place: output injection, clipboard lifecycle (including WinRT “exclude from history”), and selection probing.
  - Suggested splits:
    - `text/clipboard.rs` (set/read/restore, platform-specific WinRT)
    - `text/inject.rs` (enigo key injection + output modes + lock)
    - `text/selection_probe.rs` (copy/insert/clipboard-only strategies)
  - Acceptance hint: keep the public functions and `#[tauri::command]` signatures in `commands/text.rs` as wrappers so callers don’t change.

### Frontend (React/TS)

- **Split `app/src/components/settings/PromptSettings.tsx` (~253KB).**
  - Why: it’s doing UI layout *and* business logic for presets/router/Quick Ask/Quick Replace.
  - Suggested splits: presets editor, router panel, quick ask panel, quick replace panel, plus 1–2 hooks that own the data plumbing.

- **Split `app/src/lib/tauri.ts` (~100KB).**
  - Why: it mixes “invoke wrappers”, settings normalization/migrations, and a bunch of shared types.
  - Suggested split:
    - `lib/tauri/types.ts`
    - `lib/tauri/settings.ts` (get/normalize/update + emit `settings-changed`)
    - `lib/tauri/commands.ts` (thin invoke wrappers)
    - (optional) `lib/tauri/events.ts` (listen/emit helpers)
  - Goal: reduce Rust/TS contract drift and make it clearer which calls must also do `configAPI.syncPipelineConfig()` / emit events.

- **Continue splitting `app/src/OverlayApp.tsx` (~81KB).**
  - This is already tracked above, but size-wise it’s still one of the top hotspots.

## Provider settings follow-ups

- **Speechmatics language configurability**
  - There’s an inline TODO in `app/src-tauri/src/stt/speechmatics.rs` to make language configurable.
  - This likely wants a UI setting + plumbing into provider construction (plus defaults + TS normalization).

## Core architecture / design improvements

These are “bigger than a ticket” changes that would make the core easier to evolve safely.

- **Create a clean layering boundary: “Tauri shell” vs “Core services”.**
  - Today: a lot of orchestration lives in `app/src-tauri/src/lib.rs` and reaches into many subsystems.
  - Goal: keep `lib.rs` focused on wiring (commands/events/windows/tray), and move real business logic into a small set of services/modules.
  - Suggested shape:
    - `core/*` (pipeline orchestration, quick ask/replace orchestration)
    - `adapters/*` (tauri emit/invoke, filesystem, audio capture, clipboard)
    - `commands/*` becomes thin wrappers around core services
  - Why this helps: it’s much easier to test “core logic” without needing a Tauri runtime.

- **Make settings a versioned, schema-driven contract (single source of truth).**
  - Today: backend seeds defaults in `ensure_default_settings(...)`, and frontend normalizes/migrates in `app/src/lib/tauri.ts`.
  - Add:
    - `settings_version` stored in `settings.json`
    - explicit migrations (vN -> vN+1) that run at startup (not when visiting a UI screen)
    - a small “settings doctor” function/command: validate -> normalize -> report problems (for debugging)
  - Bonus: this also reduces Rust/TS drift because the migration logic lives in one place.

- **Introduce a typed event contract (avoid stringly-typed event drift).**
  - Today: many events are string names with ad-hoc payloads (`pipeline-state-changed`, `overlay-audio-level`, etc.).
  - Suggested: define a single event map (name -> payload type) in one place and export it:
    - Rust: central module that emits only through typed helpers
    - TS: `events.ts` that defines the same names + payload typing (generated if possible)
  - Acceptance hint: callers can still “listen by string”, but new code should go through the typed wrapper.

- **Standardize error handling across commands (one error shape to the UI).**
  - Today: errors bubble up in different formats depending on where they come from.
  - Suggested: one `AppError` type with stable fields (code, message, details, retryable, request_id?) and a single conversion path to Tauri command errors.
  - Why: frontend error UI becomes simpler and more consistent; telemetry/logging can attach codes.

- **Dependency injection seams for hard IO (testability).**
  - You already have good “seams” in places (e.g. `with_client(...)` patterns).
  - Next step: formalize a few minimal traits/interfaces so the pipeline can be tested without:
    - CPAL devices
    - real filesystem
    - real network
  - Keep it small: only extract interfaces where unit tests would meaningfully increase confidence.

- **Document the pipeline as a state machine contract (and enforce it).**
  - Add a small transition table comment + a single “transition helper” that enforces allowed moves.
  - This complements existing guard methods and makes it harder to accidentally introduce illegal transitions.

## Prevent Rust/TS contract drift

- **Generate or validate TS types against backend schemas.**

  - The CI failures we hit were mostly “frontend types lagging behind backend reality” (e.g. new request log fields / settings keys like `quick_replace_enabled`).
  - Ideas:
    - Generate TypeScript types from the Rust structs (or from the JSON schemas in `app/src-tauri/gen/schemas/`) and import those into `app/src/lib/tauri.ts`.
    - Or add a small check that compares the settings keys expected by `tauriAPI.getSettings()` vs the keys seeded/migrated by `ensure_default_settings(...)` in Rust.
  - Goal: avoid shipping changes where Rust and TS disagree on the shape of settings/logs.

## Lint rule ratchet (Biome)

- **Re-enable stricter Biome rules gradually (ratchet).**

  - To get CI stable, we temporarily downgraded several high-churn rules to warnings in `app/biome.json`.
    - Hook dependency checks: `lint/correctness/useExhaustiveDependencies`
    - “Unknown data” typing noise: `lint/suspicious/noExplicitAny`
    - A11y rules that require larger UI refactors: `lint/a11y/*` (semantic buttons, ARIA checks, media captions)
    - Security/XSS rule that needs a more deliberate audit: `lint/security/noDangerouslySetInnerHtml`
    - Non-null assertions: `lint/style/noNonNullAssertion`
    - Some style/complexity preferences: `lint/style/useTemplate`, `lint/style/useExponentiationOperator`, `lint/complexity/useOptionalChain`
  - Follow-up approach:
    - Pick one rule at a time (e.g. `lint/correctness/useExhaustiveDependencies`) and fix the existing findings.
    - Flip it back to `error` once the repo is clean.
  - Goal: keep CI green while steadily improving quality instead of “big bang” lint migrations.

## Hotkey normalization UX

- **Decide whether `normalize_shortcut_string(...)` should output “modifiers first”.**

  - Current behavior sorts tokens alphabetically, which produces canonical strings like `"a+control"`.
  - That’s consistent and easy to test, but it’s a little “inside-out” for humans (people expect `"control+a"`).
  - Follow-up options:
    - Keep current behavior and ensure the UI always formats shortcuts for display (separate from canonical serialization), or
    - Change normalization to sort modifiers before non-modifiers (and update any persisted settings/tests accordingly).

## Prompt settings UI complexity

- **Split `app/src/components/settings/PromptSettings.tsx` into smaller components + hooks.**
  - Suggested splits: presets editor, intent router panel, Quick Ask panel, Quick Replace panel, and shared “provider/model” sections.
  - Goal: reduce file size, simplify hook dependencies, and make it easier to test isolated sections.

## Rust clippy warning backlog

- **Chip away at the clippy warnings so `cargo clippy` is more signal than noise.**

  - `pnpm -C app check:ci` currently passes, but clippy emits a lot of warnings, which makes it harder to spot new issues.
  - Suggestion: add a gradual cleanup list (start with low-risk mechanical fixes like `unwrap_or_default`, `manual_clamp`, `needless_return`, and the duplicated Windows cfg attribute).

- **Deal with `clippy::too_many_arguments` via “args structs” (not a drive-by).**

  - Some warnings (e.g. `run_capture_thread(...)` in `audio_capture.rs`, `iterate_rewrite_prompt(...)` in `commands/llm.rs`, and a few functions in `pipeline.rs`) likely need a small refactor.
  - Suggested pattern: replace long parameter lists with a single `struct` argument (e.g. `RunCaptureThreadArgs { ... }`).
  - This improves readability *and* makes call-sites less error-prone, but it’s bigger than a pure mechanical clippy fix.
  - Tracking plan: see `docs/Plans/CLIPPY_WARNING_CLEANUP_PLAN.md` (Batch 5).

## Rust deterministic testing seams (hard IO audit)

- **Audio device IO (CPAL):**

  - Hot spots:
    - `app/src-tauri/src/audio_capture.rs` (CPAL host/device/stream + callback threading)
    - `app/src-tauri/src/commands/audio.rs` (device listing, “ensure active stream” for meters)
    - `app/src-tauri/src/pipeline.rs` (references to CPAL device selection + meter updates)
  - Small test seams:
    - Keep CPAL behind a tiny trait (e.g. “AudioCaptureBackend”) so pipeline state transitions can be tested with a fake capture backend that emits deterministic “audio level” events.
    - Push more logic into pure helpers (device ID parsing/normalization, “what should happen when device missing”) and unit-test those without needing a CPAL host.

- **Filesystem IO (history/recordings/stats/backups/models):**

  - Hot spots:
    - `app/src-tauri/src/history.rs` (read/write history file, metadata checks)
    - `app/src-tauri/src/recordings.rs` (create/read/delete/list recordings)
    - `app/src-tauri/src/stats.rs` and `app/src-tauri/src/commands/stats.rs` (append logs, list/delete)
    - `app/src-tauri/src/commands/backup.rs` (read/write settings backup)
    - `app/src-tauri/src/commands/whisper.rs` + `app/src-tauri/src/commands/config.rs` (model dir creation/download temp files)
  - Small test seams:
    - Centralize “app data path” + “ensure_dir” helpers so tests can point everything at a temp dir without reaching into many modules.
    - Consider a minimal “Fs” interface only where needed (read/write/list/delete) for the most critical code paths (history + recordings), but avoid big refactors.

- **Network IO (reqwest providers + proxy config):**

  - Hot spots:
    - `app/src-tauri/src/network.rs` + `app/src-tauri/src/commands/network.rs` (proxy + custom cert loading)
    - Providers under `app/src-tauri/src/{llm,stt,embeddings}/**` (reqwest calls)
  - Small test seams:
    - Continue the existing pattern of `with_client(...)` constructors (already present in several providers) so tests can inject a preconfigured client.
    - Prefer a **base URL override** (defaulting to production) for providers that hardcode endpoints, so Wiremock contract tests can target a local server.

## A11y lint follow-ups

- **Audit and reduce inline Biome a11y ignores added during the “0 warnings now” push.**

  - Some UI patterns are genuinely constrained (e.g., interactive elements inside Mantine `Accordion.Control`, which renders as a `<button>` and can’t legally contain nested `<button>`s).
  - But where we used ignores as a pragmatic workaround, a follow-up could:
    - refactor nested interactive regions to avoid button-in-button structure,
    - replace `role="button"` containers with real buttons where valid,
    - and re-evaluate `lint/a11y/useMediaCaption` for the audio-test UI (captions likely not applicable, but confirm intent).

## Ralph harness (Copilot CLI) ergonomics

- **Remove hard-coded profile `ValidateSet` and discover profiles dynamically.**

  - Today the harness scripts list `kolboo` in a `ValidateSet`, which means adding a new profile requires editing scripts.
  - Follow-up idea: accept any `-Profile` string, then validate by checking for `ralph/<profile>/profile.json` (or legacy `ralph/profiles/<profile>.json`) and show a friendly error listing available profiles.
  - Bonus: add a `List-Profiles` helper command/script.
