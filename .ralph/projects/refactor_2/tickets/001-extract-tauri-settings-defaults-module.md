# Ticket: Extract Tauri settings defaults/migrations into a module

## Goal (what we want)

Reduce risk and review overhead by moving the settings seeding + migration helpers out of the giant `app/src-tauri/src/lib.rs` into a small, focused module, without changing behavior.

- We want: `lib.rs` to be more “wiring”, less “business logic”.
- So that: future settings changes are safer and faster to review.

## Context (what exists today)

- Hot spot file: `app/src-tauri/src/lib.rs` (very large; mixes app bootstrap + settings + pipeline orchestration).
- Settings defaults + migrations are currently implemented inline via `ensure_default_settings(...)` (and helpers nearby).
- The repo conventions call out that if we add/rename settings we need to touch both Rust defaults/migrations and TS normalization.

## Acceptance criteria (how we know it’s done)

- [ ] Create a new module for settings defaults/migrations (suggested path: `app/src-tauri/src/settings/defaults.rs` + `app/src-tauri/src/settings/mod.rs`).
- [ ] Move `ensure_default_settings(...)` and its closely-related helper functions/types into that module.
- [ ] Update `lib.rs` to call into the new module (thin wrapper only).
- [ ] No runtime behavior changes:
  - same settings keys and default values
  - same migration behavior
- [ ] Rust + TS typecheck still pass.

## Edge cases / gotchas

- Watch out for implicit ordering: some defaults may depend on earlier inserts.
- Keep file paths / store keys identical (stringly-typed drift risk).
- Avoid introducing new cyclic module imports.

## Non-goals (explicitly out of scope)

- No redesign of settings schema.
- No changes to the TS settings normalization layer.
- No “split all of lib.rs” in one go.

## Notes / hints

- Keep this ticket “move code + small compile fixes only”.
