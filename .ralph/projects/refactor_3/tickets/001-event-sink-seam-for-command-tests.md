# Ticket: (High) Introduce an EventSink seam for command/orchestration tests

## Goal (what we want)

Make backend command/orchestration logic testable without spinning up a full Tauri runtime by introducing a tiny event emission abstraction.

- We want: unit tests that can assert “this command emitted event X with payload Y”.
- So that: we can add deterministic Rust tests for orchestration/state transitions without E2E.

## Context (what exists today)

- The backend emits events through Tauri handles, which makes pure unit tests hard.
- There’s already a refactor note suggesting an `EventSink` trait seam.
- Likely relevant areas:
  - `app/src-tauri/src/lib.rs` (command wiring)
  - places that call `app.emit(...)`, `window.emit(...)`, or similar

## Acceptance criteria (how we know it’s done)

- [ ] Introduce a small Rust trait (suggested name: `EventSink`) that can emit events by name + payload.
  - Keep it deliberately tiny (1-2 methods).
- [ ] Provide a production implementation that wraps the existing Tauri emitter(s) (e.g. `AppHandle` / `Window`).
- [ ] Provide a test implementation (e.g. collects emitted events into a `Vec`) that can be asserted on.
- [ ] Refactor **one** existing command or orchestration path to use the `EventSink` seam (don’t try to convert everything).
- [ ] Add at least **one fast unit test** that:
  - triggers that path
  - asserts the expected event(s) were emitted
- [ ] No runtime behavior changes: event names and payloads must remain identical.

## Edge cases / gotchas

- Keep payload typing ergonomic: if the real code emits serializable structs, the seam should not force “stringly typed JSON everywhere”.
- Avoid adding lifetimes/trait object complexity unless needed; simplest thing that works.
- Don’t accidentally change event names (contract drift risk).

## Non-goals (explicitly out of scope)

- No full conversion of every emitter call to use `EventSink`.
- No redesign of event names/payload contracts.

## Notes / hints

- Keep this a “seam + one adopter + one test” ticket.
