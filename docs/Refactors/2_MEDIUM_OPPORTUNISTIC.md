# Medium-priority refactors (opportunistic)

These are worthwhile, but I’d usually do them **when you’re already working nearby**, or when you have a dedicated cleanup window.

They tend to improve maintainability and reduce future friction, but they’re not as directly “risk reducing” as the high-impact list.

## Biggest “hot spot” files by size (worth refactoring)

These are the files that are _currently_ the largest / most responsibility-dense. They aren’t “bad”, but they’re the most likely to become painful to change.

### Rust backend

- **Split `app/src-tauri/src/lib.rs` (very large).**
  - Why: it mixes app bootstrap, tray/window behavior, hotkeys, settings seeding/migration, pipeline orchestration, Quick Ask/Replace wiring, and event emission.
  - Suggested splits:
    - `bootstrap/*` (plugins, window creation, menu/tray setup)
    - `shortcuts/*` (global shortcut registration + Escape-to-cancel lifecycle)
    - `sessions/*` (record start/stop orchestration; Quick Ask / Quick Replace branches)
    - `settings/defaults.rs` (keep `ensure_default_settings(...)` + migrations close to settings types)
    - `overlay/*` (show/hide/position logic)
  - Acceptance hint: keep the public Tauri command API the same; this is mostly moving code + adding thin wrappers.

- **Split `app/src-tauri/src/pipeline.rs` (~188KB).**
  - Why: it contains unrelated concerns (provider construction/caching, routing logic, state machine + config, embedding cache/persistence, and helper utilities).
  - Suggested splits:
    - `pipeline/state.rs` (state machine + transitions/guards)
    - `pipeline/config.rs` (PipelineConfig defaults + normalization)
    - `pipeline/providers.rs` (STT/LLM provider creation + caching)
    - `pipeline/router/*` (embeddings router + LLM router + diagnostics payload building)
  - Bonus: lots of helper functions here are pure (e.g. path normalization / routing scoring) and can get fast unit tests once extracted.

### Frontend (React/TS)

- **Continue splitting `app/src/OverlayApp.tsx` if it grows again.**
  - Goal: keep overlay UI logic testable and predictable.

- **Turn `app/src/components/settings/PromptSettings.tsx` into a thin page shell.**
  - Why: it mixes UI rendering, state wiring, and backend mutations for many distinct settings sections.
  - Ideal state: the file reads like a table of contents, with small focused subcomponents and a single place for shared state and orchestration.
  - Step‑by‑step (for Dovie):
   1) ✅ **Extract UI‑only sections** into their own components under `app/src/components/settings/prompt/`.
     - Done: extracted `PromptIntentRouterSection` (router UI).
     - Next: Quick Ask panel wrapper, Quick Replace panel wrapper, STT provider UI.
     - Keep their props “dumb”: pass values + callbacks, no direct data fetching.
   2) ✅ **Create a shared “profile state” hook** (e.g., `usePromptSettingsProfileState.ts`).
     - Done: `usePromptSettingsProfileState` now owns profile-local state + sync effect.
     - Exposes values + setters for profile overrides/inheritance state.
   3) ✅ **Move prompt test logic** (rewrite test + STT test) into a `usePromptSettingsTests.ts` hook.
     - Done: rewrite + STT test state and runners moved into `usePromptSettingsTests`.
   4) ✅ **Extract "provider/model selection" logic** into a `usePromptProviderOptions.ts` helper.
     - Done: provider dropdown options, effective providers/models, model options, default profile values, and model queries moved into hook.
     - Also exports `getLlmModelOptionsForProvider` helper used by UI callbacks.
   5) ✅ **Introduce a `PromptSettingsLayout` component** for the accordion/sections layout.
     - Done: created `RewriteSettingsSection.tsx` (~880 lines) and integrated into `PromptSettings.tsx`.
     - Reduced `PromptSettings.tsx` from ~3700 to ~2745 lines (~955 line reduction).
     - Created 16 handler functions to wrap `openDisableOverrideDialog` + state setters + profile save calls.
     - Next: continue extracting more sections (Presets, Quick Ask/Replace) to further reduce file size.
   6) 🚧 **Trim `PromptSettings.tsx` imports** after each extraction.
     - Goal: the file should be short (roughly <400 lines) and mostly a render skeleton.
     - Progress: file is now ~2745 lines; still needs more extraction work.
   7) **Add/adjust tests where logic moved** (hooks or utilities).
     - Focus on pure logic (no network, no API keys).
  - Acceptance hint: the main file should be readable top‑to‑bottom without scrolling for minutes; detailed logic lives in hooks/components.

## Overlay UI (React)

- **Consolidate overlay state into a single “overlay controller” object (as needed).**
  - Today some state lives in refs, some in `useState`, some in the reducer.
  - A follow-up could consolidate more of this into one predictable state machine.
