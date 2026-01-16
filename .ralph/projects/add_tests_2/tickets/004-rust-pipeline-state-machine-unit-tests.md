# Ticket: Add deterministic pipeline state-machine tests (no audio devices)

## Goal (what we want)

Add a small set of deterministic Rust tests that validate critical pipeline state transitions without requiring CPAL devices or real audio.

- We want: tests that prove the state machine does the right thing.
- So that: pipeline changes don’t silently break recording/cancel flows.

## Context (what exists today)

- Core state machine: `app/src-tauri/src/pipeline.rs`
- Existing tests: `app/src-tauri/src/tests/pipeline_edge_case_tests.rs` (and friends).
- Audio capture is CPAL-based and should not be required for these tests.

## Acceptance criteria (how we know it’s done)

- [ ] Add at least 3 focused tests that assert state transitions (Given/When/Then style).
- [ ] Tests must not touch real devices or network.
- [ ] If the pipeline currently couples too tightly to CPAL, introduce the smallest possible seam (trait or injected dependency) that is behavior-preserving.
- [ ] Tests pass on Windows.

## Edge cases / gotchas

- Cancellation paths are part of UX; test at least one “cancel during X” flow.
- Avoid tests that depend on timers/sleeps.

## Non-goals (explicitly out of scope)

- End-to-end UI automation.
- Large pipeline refactors.
