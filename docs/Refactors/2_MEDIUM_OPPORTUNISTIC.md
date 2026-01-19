# Medium-priority refactors (opportunistic)

These are worthwhile, but I’d usually do them **when you’re already working nearby**, or when you have a dedicated cleanup window.

They tend to improve maintainability and reduce future friction, but they’re not as directly “risk reducing” as the high-impact list.

## Overlay UI (React)

- **Consider a single “overlay controller” state object.**
  - Right now some state lives in refs, some in `useState`, some in the reducer.
  - A follow-up could consolidate more of this into one predictable state machine, but that’s a larger change.

## Biggest “hot spot” files by size (worth refactoring)

These are the files that are _currently_ the largest / most responsibility-dense. They aren’t “bad”, but they’re the most likely to become painful to change.

### Rust backend

- **Split `app/src-tauri/src/pipeline.rs` (~188KB).**
  - Why: it contains unrelated concerns (provider construction/caching, routing logic, state machine + config, embedding cache/persistence, and helper utilities).
  - Suggested splits:
    - `pipeline/state.rs` (state machine + transitions/guards)
    - `pipeline/config.rs` (PipelineConfig defaults + normalization)
    - `pipeline/providers.rs` (STT/LLM provider creation + caching)
    - `pipeline/router/*` (embeddings router + LLM router + diagnostics payload building)
  - Bonus: lots of helper functions here are pure (e.g. path normalization / routing scoring) and can get fast unit tests once extracted.

### Frontend (React/TS)

- **Continue splitting `app/src/OverlayApp.tsx` (~81KB).**
  - This is already tracked above, but size-wise it’s still one of the top hotspots.

## Testing “ideal state” follow-ups (optional)

- **Raise TS per-file coverage thresholds gradually.**
  - Current: we’ve started ratcheting the Tauri client modules upward (e.g. `app/src/lib/tauri/commands.ts`, `app/src/lib/tauri/settings.ts`).
  - Next suggested targets from the plan: **80% lines**, **70% branches** (only if the churn feels manageable).
