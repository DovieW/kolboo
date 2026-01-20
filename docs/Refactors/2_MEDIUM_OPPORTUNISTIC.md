# Medium-priority refactors (opportunistic)

These are worthwhile, but I'd usually do them **when you're already working nearby**, or when you have a dedicated cleanup window.

They tend to improve maintainability and reduce future friction, but they're not as directly "risk reducing" as the high-impact list.

## Biggest "hot spot" files by size (worth refactoring)

These are the files that are _currently_ the largest / most responsibility-dense. They aren't "bad", but they're the most likely to become painful to change.

### Rust backend

- **Continue shrinking `app/src-tauri/src/lib.rs` (still large).**
  - Why: large files are harder for AI agents to edit correctly (context window limits, harder to find the right spot). Deduplication means fewer places to update when changing behavior.
  - Goal: make `lib.rs` a thin "wiring" layer; move duplicated/cohesive logic into focused modules.

  - **Done:**
    - [x] Create `sessions/` module scaffold (`app/src-tauri/src/sessions/mod.rs`).
    - [x] Extract shared Quick Ask window helpers (`sessions/quick_ask.rs`).
    - [x] Centralize Quick Ask event constants (`EVENT_QUICK_ASK_*`) and window label.
    - [x] Unify selection probes into `sessions/selection_probe.rs`.
    - [x] Centralize remaining event constants in `events.rs`.

  - **Remaining (each provides real dedup or separation value):**
    - *(No high-value items remaining for lib.rs - only large code moves without deduplication.)*

  - **Not doing (moving code without dedup is just churn):**
    - ~~Extract recording orchestration~~ - the main recording flow isn't duplicated; moving it just relocates complexity.
    - ~~Extract history/log updates~~ - these are already small helper calls; wrapping them in another module adds indirection without removing code.
    - ~~Extract Quick Ask answer flow~~ - while large (~560 lines), it's a single inline block with many dependencies; moving it doesn't reduce code or dedup.
    - ~~Extract Quick Replace rewrite flow~~ - same reasoning; cohesive but not duplicated.

- **Split `app/src-tauri/src/pipeline.rs` (~188KB).**
  - Why: it contains unrelated concerns (provider construction/caching, routing logic, state machine + config, embedding cache/persistence, and helper utilities).
  - Suggested splits:
    - `pipeline/state.rs` (state machine + transitions/guards)
    - `pipeline/config.rs` (PipelineConfig defaults + normalization)
    - `pipeline/providers.rs` (STT/LLM provider creation + caching)
    - `pipeline/router/*` (embeddings router + LLM router + diagnostics payload building)

