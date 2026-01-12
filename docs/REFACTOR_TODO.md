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
