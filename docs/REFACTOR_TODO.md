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
