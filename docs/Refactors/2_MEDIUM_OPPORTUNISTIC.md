# Medium-priority refactors (opportunistic)

These are worthwhile, but I’d usually do them **when you’re already working nearby**, or when you have a dedicated cleanup window.

They tend to improve maintainability and reduce future friction, but they’re not as directly “risk reducing” as the high-impact list.

## Biggest “hot spot” files by size (worth refactoring)

These are the files that are _currently_ the largest / most responsibility-dense. They aren’t “bad”, but they’re the most likely to become painful to change.

### Rust backend

- **Continue shrinking `app/src-tauri/src/lib.rs` (still large).**
  - Why: it’s still a very responsibility-dense file, even after prior extractions.
  - Good next slice: extract the recording/Quick Ask/Quick Replace orchestration into a `sessions/*` module so `lib.rs` trends toward “wiring only”.
  - Progress:
    - Extracted shared Quick Ask helpers (emit + ensure window visible) into `app/src-tauri/src/sessions/quick_ask.rs`.
    - Centralized Quick Ask event name constants so we stop duplicating magic strings.
  - Remaining:
    - Move the actual Quick Ask / Quick Replace / recording orchestration blocks out of `lib.rs` incrementally (keep commits small).

  - Step-by-step path to “`lib.rs` is wiring only” (do these in small slices):
    - [x] Create `sessions/` module scaffold (`app/src-tauri/src/sessions/mod.rs`).
    - [x] Extract *shared* Quick Ask window helpers (`sessions/quick_ask.rs`).
    - [x] Centralize Quick Ask event constants (`EVENT_QUICK_ASK_*`) and window label.
    - [ ] Extract “Quick Ask selection probe” into `sessions/quick_ask_probe.rs` (or `sessions/selection_probe.rs`).
      - Goal: `lib.rs` shouldn’t know about epochs/locks/sentinels.
      - Acceptance: `lib.rs` calls something like `quick_ask::spawn_selection_probe(...)`.
    - [ ] Extract “Quick Replace selection probe” into the same probe module (shared impl, different config).
      - Acceptance: both probes use one helper with a small config struct.
    - [ ] Extract “Quick Ask answer generation” into `sessions/quick_ask_session.rs`.
      - Inputs: question text, selected/clipboard context (optional), effective provider config.
      - Output: emits started/answer events + request log updates (all in one place).
    - [ ] Extract “Quick Replace rewrite selection” into `sessions/quick_replace_session.rs`.
      - Acceptance: `lib.rs` just chooses the branch and passes inputs.
    - [ ] Extract “recording stop -> pipeline run -> output/history/logs” orchestration into `sessions/recording_session.rs`.
      - Keep it surgical: start by extracting a single helper like `handle_pipeline_result_success(...)`.
    - [ ] Move “emit pipeline events” + “history updates” into tiny helpers/modules (only if it reduces duplication).
      - Rule: don’t create a new abstraction unless it removes repeated code.
    - [ ] Final cleanup pass: `lib.rs` should mostly contain:
      - module declarations
      - Tauri setup/registration (commands/events/windows/tray)
      - small type exports
      - *no* long async flows
      - *no* request-log/history business logic

- **Split `app/src-tauri/src/pipeline.rs` (~188KB).**
  - Why: it contains unrelated concerns (provider construction/caching, routing logic, state machine + config, embedding cache/persistence, and helper utilities).
  - Suggested splits:
    - `pipeline/state.rs` (state machine + transitions/guards)
    - `pipeline/config.rs` (PipelineConfig defaults + normalization)
    - `pipeline/providers.rs` (STT/LLM provider creation + caching)
    - `pipeline/router/*` (embeddings router + LLM router + diagnostics payload building)

