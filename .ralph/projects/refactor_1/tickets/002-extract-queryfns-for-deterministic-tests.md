# Ticket: Extract TanStack Query queryFns into pure helpers

## Goal (what we want)

Make the data-fetching logic testable and deterministic by extracting TanStack Query `queryFn`s into pure helper functions, then adding unit tests for those helpers.

- We want: tests that validate the core logic without mounting React.
- So that: changes to backend contracts/settings don’t silently break UI data paths.

## Context (what exists today)

- Current file: `app/src/lib/queries.ts`
- It wires `tauriAPI` calls into TanStack Query hooks.
- The repo refactor parking lot suggests extracting pure `queryFn` helpers.

## Acceptance criteria (how we know it’s done)

- [ ] Extract query functions into something like:
  - `app/src/lib/queries/queryFns.ts` (pure functions that accept `tauriAPI` as a parameter)
  - `app/src/lib/queries.ts` becomes thin wrappers calling those helpers
- [ ] Add Vitest unit tests for the extracted helpers (no network, no keys).
- [ ] Tests should mock `tauriAPI` and validate transforms/edge cases.

## Edge cases / gotchas

- Keep tests deterministic (no timers/sleeps).
- Don’t accidentally create module singleton state that leaks between tests.

## Non-goals (explicitly out of scope)

- No UI component tests.
- No changes to query cache keys unless required (prefer zero behavior change).

## Notes / hints

- Prefer testing exported helpers (public behavior), not hook internals.
