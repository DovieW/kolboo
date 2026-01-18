# Ticket: Extract PipelineConfig into a dedicated module

## Goal (what we want)

Make the Rust pipeline code easier to reason about and test by extracting `PipelineConfig` (defaults + normalization) out of `app/src-tauri/src/pipeline.rs` into a dedicated module.

- We want: pipeline config logic isolated from the pipeline state machine/runtime orchestration.
- So that: config changes are lower-risk and get faster unit tests.

## Context (what exists today)

- Hot spot file: `app/src-tauri/src/pipeline.rs` (very large; multiple concerns).
- `PipelineConfig` and its defaulting/normalization logic are mixed in with runtime pipeline code.

## Acceptance criteria (how we know it’s done)

- [ ] Create `app/src-tauri/src/pipeline/config.rs` (and `app/src-tauri/src/pipeline/mod.rs` if needed).
- [ ] Move `PipelineConfig` (and only the config-related helpers) into `pipeline/config.rs`.
- [ ] Keep the public API stable for existing callers:
  - either re-export from `pipeline.rs`, or update imports with minimal churn.
- [ ] Add at least 1 fast unit test for a pure config normalization behavior (if such a helper exists today). If there is no clean pure helper yet, extract one *small* pure helper and test it.
- [ ] No runtime behavior changes.

## Edge cases / gotchas

- Avoid moving anything that touches async runtime / state machine transitions.
- Ensure serde defaults (if any) are preserved.
- Watch for private module visibility (`pub(crate)` vs `pub`).

## Non-goals (explicitly out of scope)

- No refactor of the pipeline state machine.
- No provider/router code movement.

## Notes / hints

- The best ROI is to keep this a “pure move + 1 tiny test” ticket.
