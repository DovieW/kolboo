# Plan: split `app/src-tauri/src/lib.rs` into modules (behavior-preserving)

This plan is for Dovie to refactor the Tauri backend entrypoint without changing the public command/event contract.

## Why this plan exists

`app/src-tauri/src/lib.rs` is currently a “mega file” that mixes multiple responsibilities:

- app bootstrap + plugin setup
- window/tray/menu wiring
- global shortcut lifecycle (including escape-to-cancel)
- settings seeding/migrations (`ensure_default_settings(...)`)
- pipeline orchestration glue
- lots of event emission

That combination makes every future change slower, riskier, and more conflict-prone.

**The ROI:** once the file is split, most future backend changes become “touch one focused module” work instead of “edit the big ball of string and hope you didn’t tug the wrong thread”.

## Goals (what we want)

- Keep **public behavior identical** (same commands, same event names/payloads, same settings keys/defaults/migrations).
- Reduce `lib.rs` to mostly **wiring** (register commands, call into modules).
- Create clear ownership boundaries so future refactors/testing seams are easier.
- Keep the refactor incremental: small commits, easy to review, easy to revert.

## Non-goals (explicitly out of scope)

- Changing command names, signatures, or UI↔backend contract.
- Redesigning the pipeline/state machine.
- “Perfect architecture” or introducing dependency injection everywhere.
- Reformatting unrelated code.

## Ground rules (how we keep it safe)

1. **Move code, don’t rewrite it.** Prefer copy/move + compile fixes.
2. **One concern per extraction.** Don’t bundle 5 moves into one PR chunk.
3. **Keep string constants stable.** Event names and settings keys are the contract.
4. **Verify often.** After each extraction chunk, run the repo’s CI gate:
	- `pnpm -C app check:ci`
5. **Minimize visibility changes.** Use `pub(crate)` where possible; avoid making things fully `pub` unless needed.

## Target end-state (shape)

Keep `app/src-tauri/src/lib.rs` as the entrypoint, but move most logic into modules like:

- `app/src-tauri/src/bootstrap/*`
	- plugin setup
	- window creation helpers
	- tray/menu setup
- `app/src-tauri/src/settings/*`
	- `defaults.rs`: `ensure_default_settings(...)` + migrations
- `app/src-tauri/src/shortcuts/*`
	- global shortcut registration
	- escape-to-cancel lifecycle
- `app/src-tauri/src/overlay/*`
	- show/hide/position helpers
	- overlay event emission helpers
- `app/src-tauri/src/commands/*`
	- optional: if `lib.rs` currently contains command implementations directly

This is not sacred. The key is: each module has one responsibility.

## Step-by-step approach (recommended order)

### Step 0 — Prep (no behavioral change)

- Snapshot the “known good” state:
	- Make sure `pnpm -C app check:ci` is green before you start (or note existing failures separately).
- Add a tiny comment at the top of `lib.rs` listing the extraction buckets (so future-you remembers the plan).

**Exit criteria:** build/tests are green; no refactor started yet.

### Step 1 — Extract settings defaults/migrations (highest ROI + safest)

- Create `app/src-tauri/src/settings/mod.rs` + `app/src-tauri/src/settings/defaults.rs`.
- Move `ensure_default_settings(...)` and its close helpers into `settings/defaults.rs`.
- In `lib.rs`, replace the implementation with a thin call:
	- `settings::defaults::ensure_default_settings(...)`.

**Verification:** `pnpm -C app check:ci`

**Why first:** it’s mostly pure-ish logic + store IO; fewer tricky lifetime/runtime interactions.

### Step 2 — Extract “overlay wiring” helpers

- Create `app/src-tauri/src/overlay/mod.rs` (and submodules if needed).
- Move overlay show/hide/position logic and overlay-specific emit helpers.
- Keep event names/payload shapes identical.

**Verification:** `pnpm -C app check:ci`

### Step 3 — Extract shortcuts lifecycle

- Create `app/src-tauri/src/shortcuts/mod.rs`.
- Move global shortcut registration + escape-to-cancel lifecycle into this module.
- Keep the same lock/async pattern (avoid re-entrant registration).

**Verification:** `pnpm -C app check:ci`

### Step 4 — Extract bootstrap/wiring chunks

- Create `app/src-tauri/src/bootstrap/mod.rs`.
- Move:
	- plugin initialization
	- window creation helpers
	- tray/menu setup
- Keep `run()` (or equivalent) in `lib.rs`, but make it call into `bootstrap::*` functions.

**Verification:** `pnpm -C app check:ci`

### Step 5 — Optional: extract command implementation modules (only if it buys clarity)

If `lib.rs` contains large command bodies:

- Move each command’s implementation into `app/src-tauri/src/commands/<area>.rs`.
- Keep command registration in `lib.rs`.
- Avoid changing command signatures.

**Verification:** `pnpm -C app check:ci`

## “Don’t accidentally break the contract” checklist

Before you consider the refactor “done”, do a quick pass for these:

- Settings keys:
	- no changed strings in defaults/migrations
	- same default values
- Events:
	- no renamed event strings
	- payload structs unchanged
- Commands:
	- no renamed `#[tauri::command]` functions
	- no signature changes

If you want extra confidence, re-run the TS/Rust contract tests (they’re part of `check:ci`).

## Suggested commit strategy

Make small, boring commits that are easy to review:

- `refactor(tauri): extract settings defaults module`
- `refactor(tauri): extract overlay helpers module`
- `refactor(tauri): extract shortcuts module`
- `refactor(tauri): extract bootstrap module`

Each commit should keep `check:ci` green.

## Risks / gotchas

- Hidden initialization order coupling: some setup might assume something was created earlier.
- Module visibility: moving code may require exposing types; prefer `pub(crate)` and re-export sparingly.
- Accidental string drift: event names and settings keys must not change.

## Stretch goal (after the refactor)

Once this is merged and stable, you can consider adding a test seam for event emission (an `EventSink` trait) so command/orchestration logic can be tested without a full Tauri runtime. That’s a separate ticket.
