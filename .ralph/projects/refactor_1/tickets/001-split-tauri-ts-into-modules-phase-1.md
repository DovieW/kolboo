# Ticket: Split tauri.ts into modules (phase 1)

## Goal (what we want)

Reduce churn and contract drift risk by splitting the large `app/src/lib/tauri.ts` file into smaller, single-responsibility modules, without changing the public API used by the UI.

- We want: smaller files with clearer ownership (commands vs settings vs types).
- So that: future changes are safer, easier to review, and easier to test.

## Context (what exists today)

- Hot spot file: `app/src/lib/tauri.ts` (~100KB, mixed concerns)
- It currently mixes:
  - invoke wrappers (commands)
  - settings normalization/migration
  - shared types
  - assorted helpers/events

## Acceptance criteria (how we know it’s done)

- [ ] Create a folder `app/src/lib/tauri/` and extract at least these modules:
  - `app/src/lib/tauri/commands.ts` (thin `invoke(...)` wrappers)
  - `app/src/lib/tauri/settings.ts` (get/normalize/update settings + emitting `settings-changed` as needed)
  - `app/src/lib/tauri/types.ts` (shared exported types)
- [ ] Keep `app/src/lib/tauri.ts` as a compatibility “barrel” that re-exports the same symbols as before (no call-site changes required).
- [ ] Ensure any settings updates that affect runtime behavior still persist to store _and_ call `configAPI.syncPipelineConfig()` (no regressions).
- [ ] Typecheck passes.

## Edge cases / gotchas

- Avoid circular imports (e.g. `settings.ts` importing from the barrel).
- Watch for side effects in module top-level code; keep it behavior-preserving.
- Keep event names/payload shapes unchanged.

## Non-goals (explicitly out of scope)

- No semantic behavior changes.
- No big renames of existing exported functions/types.

## Notes / hints

- Prefer moving code first, then adding tiny follow-up cleanups only if needed for types/lint.
