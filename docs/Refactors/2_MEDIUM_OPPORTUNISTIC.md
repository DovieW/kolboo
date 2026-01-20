# Medium-priority refactors (opportunistic)

These are worthwhile, but I’d usually do them **when you’re already working nearby**, or when you have a dedicated cleanup window.

They tend to improve maintainability and reduce future friction, but they’re not as directly “risk reducing” as the high-impact list.

## Biggest “hot spot” files by size (worth refactoring)

These are the files that are _currently_ the largest / most responsibility-dense. They aren’t “bad”, but they’re the most likely to become painful to change.

### Rust backend

- **Continue shrinking `app/src-tauri/src/lib.rs` (still large).**
  - Why: it’s still a very responsibility-dense file, even after prior extractions.
  - Good next slice: extract the recording/Quick Ask/Quick Replace orchestration into a `sessions/*` module so `lib.rs` trends toward “wiring only”.

- **Split `app/src-tauri/src/pipeline.rs` (~188KB).**
  - Why: it contains unrelated concerns (provider construction/caching, routing logic, state machine + config, embedding cache/persistence, and helper utilities).
  - Suggested splits:
    - `pipeline/state.rs` (state machine + transitions/guards)
    - `pipeline/config.rs` (PipelineConfig defaults + normalization)
    - `pipeline/providers.rs` (STT/LLM provider creation + caching)
    - `pipeline/router/*` (embeddings router + LLM router + diagnostics payload building)

## Overlay UI (React)

- **Consolidate overlay state into a single “overlay controller” object (as needed).**
  - Today some state lives in refs, some in `useState`, some in the reducer.
  - A follow-up could consolidate more of this into one predictable state machine.
