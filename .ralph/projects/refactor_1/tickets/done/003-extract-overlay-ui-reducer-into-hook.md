# Ticket: Extract overlay UI reducer into a dedicated hook

## Goal (what we want)

Reduce complexity in `app/src/OverlayApp.tsx` by extracting the overlay UI reducer + action types into a dedicated module/hook.

- We want: a smaller OverlayApp that focuses on wiring and rendering.
- So that: overlay behavior changes are safer and easier to reason about.

## Context (what exists today)

- Hot spot file: `app/src/OverlayApp.tsx` (large, many responsibilities)
- There is an existing refactor idea to move the reducer into `app/src/lib/useOverlayUiReducer.ts`.

## Acceptance criteria (how we know it’s done)

- [ ] Extract the reducer + action types into `app/src/lib/useOverlayUiReducer.ts` (or equivalent), exporting a hook that OverlayApp uses.
- [ ] Keep behavior identical (no UX changes).
- [ ] Add a short transition-table comment describing how the UI should behave when:
  - hotkey fires before `pipeline-state-changed`
  - polling returns a stale state
  - recording-only mode hides right after going idle
- [ ] Typecheck passes.

## Edge cases / gotchas

- Be careful about stale closures when moving logic into a hook.
- Avoid re-render loops (keep reducer pure).

## Non-goals (explicitly out of scope)

- No big overlay component split (that can be a separate ticket).
- No styling changes.
