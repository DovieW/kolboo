# How to maintain: Prevent Rust/TS contract drift

This doc is for Dovie (and future-us) to maintain the “contract drift prevention” system.

## What this system is (super plain English)

Kolboo has two parts that must agree with each other:

- **Rust backend (Tauri):** produces events, commands, and persisted settings.
- **TypeScript frontend (React):** listens to events, calls commands, and reads/writes settings.

When Rust changes a field name, event name, or settings key, TypeScript won’t automatically know.

This system exists so those mismatches cause a **clear test failure in CI**, instead of becoming a weird runtime bug.

## The moving parts (where to look)

### 1) JSON schemas generated from Rust

- Location: `app/src-tauri/gen/schemas/**`
- Purpose: “source of truth” artifacts describing the backend contract (payload/response shapes, enums, etc.).

### 2) TypeScript types + Tauri wrappers

- Main file today: `app/src/lib/tauri.ts`
- Purpose:
	- Central place for shared types (payloads, responses, settings shapes).
	- Central place for `tauriAPI.*` wrappers.

### 3) Contract tests

- Current starting point: `app/src/lib/settingsContract.test.ts`
- Purpose:
	- Prevent drift between TS types and Rust schemas.
	- Prevent drift between Rust seeded settings keys and what TS returns from `tauriAPI.getSettings()`.

(Over time, this will likely be split into smaller tests under something like `app/src/lib/contracts/**`.)

### 4) Plan doc

- See: `docs/Plans/PREVENT_RUST_TS_CONTRACT_DRIFT_PLAN.md`
- Purpose: explains *what we’re building toward* and what’s still missing.

## Day-to-day workflow: I changed Rust, what now?

### If you changed a Rust type that is part of the UI contract

Examples:

- event payloads (anything emitted to the UI)
- command request/response structs
- persisted settings shapes
- `RequestLog` fields

Do this:

1) **Regenerate the Rust schemas** (so `app/src-tauri/gen/schemas/**` matches the new Rust reality).
	- Run: `pnpm -C app schemas:generate`
2) **Update TS types** in `app/src/lib/tauri.ts` (or the appropriate TS type file).
3) **Update/extend contract tests** so they still describe the intended contract.
4) Run the test gate:
	- `pnpm -C app test`
	- and before merge: `pnpm -C app check:ci` (includes `schemas:check`)

If you forget step (1), the tests can lie (they’ll still be testing old schemas).

## Day-to-day workflow: I changed TypeScript, what now?

### If you changed a TS type that claims to match Rust

Do this:

1) Check whether there is a schema file for it under `app/src-tauri/gen/schemas/**`.
2) If yes, either:
	- adjust TS to match the schema, or
	- if TS is correct and Rust should change, make the corresponding Rust change and regenerate schemas.
3) Run `pnpm -C app test`.

## How to add a new contract item (settings / event / command)

### A) Adding a new setting key

You usually touch both sides:

- Rust: seed/migrate it in `ensure_default_settings(...)` (currently in `app/src-tauri/src/lib.rs`).
- TS: normalize it and include it in the object returned by `tauriAPI.getSettings()`.

Then:

- If the settings key is supposed to be visible to the UI, the settings key parity test should include it automatically.
- If it’s intentionally backend-only or legacy, add it to the **allowlist** in the parity test with a comment explaining why.

### B) Adding a new event

You usually touch both sides:

- Rust: emit the event with a stable name string.
- TS: listen for the event.

Best practice:

- Define the payload type once (in `app/src/lib/tauri.ts` today).
- Prefer using typed event wrappers once they exist (see the plan doc).

Also:

- Add a schema for the payload (or ensure it’s exported) so schema drift tests can validate it.
- If/when an “event-name drift” test exists, add the new event name to the TS source-of-truth event map/list.

### C) Adding a new command (invoke)

You usually touch both sides:

- Rust: add the command and its request/response types.
- TS: add an invoke wrapper (usually in `app/src/lib/tauri.ts`).

Then:

- Export a schema for the response type and add a small contract test that checks TS keys against the schema keys.

## How to interpret common failures

### “Schemas are stale” / “git diff in gen/schemas”

Meaning:

- Rust contract changed, but the schema files weren’t regenerated and committed.

Fix:

- Regenerate schemas with `pnpm -C app schemas:generate`.
- Commit the updated `app/src-tauri/gen/schemas/**` output.

### “Missing keys in schema” (TS has keys Rust schema doesn’t)

Meaning:

- TS type is claiming fields that Rust doesn’t actually output.

Fix:

- Either remove/fix the TS type field, or update Rust and regenerate schemas.

### “Missing keys in TS” (Rust schema has keys TS doesn’t)

Meaning:

- Rust added a field but TS type and/or UI code didn’t get updated.

Fix:

- Update TS type.
- Decide how the UI should handle the new field.

## Maintenance rules (so this doesn’t become a monster)

1) **Do not test everything.** Only enforce schemas that represent real UI↔backend contracts.
2) **Keep allowlists small and documented.** If something is excluded, comment why.
3) **Split tests when they get big.** Prefer multiple small tests over one mega file.
4) **Prefer boring failures.** Best failures are “schema diff” or “missing key list”, not complex logic.

## Links

- Plan: `docs/Plans/PREVENT_RUST_TS_CONTRACT_DRIFT_PLAN.md`
- Current contract tests: `app/src/lib/settingsContract.test.ts`
- Schema output folder: `app/src-tauri/gen/schemas/`
