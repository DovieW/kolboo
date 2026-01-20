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

### Frontend (React/TS)

- **Continue splitting `app/src/OverlayApp.tsx` if it grows again.**
  - Goal: keep overlay UI logic testable and predictable.

## Overlay UI (React)

- **Consolidate overlay state into a single “overlay controller” object (as needed).**
  - Today some state lives in refs, some in `useState`, some in the reducer.
  - A follow-up could consolidate more of this into one predictable state machine.
