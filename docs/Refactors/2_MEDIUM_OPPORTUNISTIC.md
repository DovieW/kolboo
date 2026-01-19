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

- **Break up `app/src-tauri/src/commands/text.rs` by responsibility.**
  - Why: it does 3 tricky OS-level jobs in one place: output injection, clipboard lifecycle (including WinRT “exclude from history”), and selection probing.
  - Suggested splits:
    - `text/clipboard.rs` (set/read/restore, platform-specific WinRT)
    - `text/inject.rs` (enigo key injection + output modes + lock)
    - `text/selection_probe.rs` (copy/insert/clipboard-only strategies)
  - Acceptance hint: keep `#[tauri::command]` signatures in `commands/text.rs` as wrappers so callers don’t change.

### Frontend (React/TS)

- **Split `app/src/components/settings/PromptSettings.tsx` (very large).**
  - Why: it’s doing UI layout *and* business logic for presets/router/Quick Ask/Quick Replace.
  - Suggested splits: presets editor, router panel, quick ask panel, quick replace panel, plus 1–2 hooks that own the data plumbing.

- **Continue splitting `app/src/OverlayApp.tsx` if it grows again.**
  - Goal: keep overlay UI logic testable and predictable.

## Overlay UI (React)

- **Extract the overlay UI reducer into a dedicated hook.**
  - Move reducer + action types into something like `app/src/lib/useOverlayUiReducer.ts`.
  - Add a short transition table comment that explains behavior when:
    - hotkey fires before `pipeline-state-changed`
    - polling returns a stale state
    - recording-only mode hides right after going idle

- **Consolidate overlay state into a single “overlay controller” object (as needed).**
  - Today some state lives in refs, some in `useState`, some in the reducer.
  - A follow-up could consolidate more of this into one predictable state machine.

## Provider settings follow-ups

- **Speechmatics language configurability.**
  - There’s an inline TODO in `app/src-tauri/src/stt/speechmatics.rs` to make language configurable.
  - Likely wants a UI setting + plumbing into provider construction (plus defaults + TS normalization).

## Testing “ideal state” follow-ups (optional)

- **Raise TS per-file coverage thresholds gradually.**
  - Treat this as a “ratchet”: only raise thresholds when tests already exist and churn is manageable.

- **Add deterministic coverage for TanStack Query logic (`app/src/lib/queries.ts`).**
  - Best approach: extract pure `queryFn` helpers and unit-test those.
  - Optional: add a per-file coverage threshold for `queries.ts` once tests exist.

- **Add at least one real “legacy settings” fixture test.**
  - Goal: validate migrations/normalization for older `settings.json` shapes.
  - Keep it deterministic (no network, no keys).

## Lint rule ratchet (Biome)

- **Re-enable stricter Biome rules gradually (ratchet).**
  - Pick one rule at a time (e.g. `lint/correctness/useExhaustiveDependencies`), fix existing findings, then flip it back to `error`.
  - Goal: keep CI green while steadily improving quality.

## Rust clippy warning backlog

- **Chip away at clippy warnings so `cargo clippy` is more signal than noise.**
  - Prefer low-risk mechanical fixes first.
  - Avoid drive-by refactors; when a refactor is needed, use “args structs” to reduce `too_many_arguments`.
