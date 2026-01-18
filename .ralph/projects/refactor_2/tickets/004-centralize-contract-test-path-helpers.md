# Ticket: Centralize contract test path helpers

## Goal (what we want)

Make contract/schema tests less brittle by centralizing the repeated “find app root + schema dir” path math into one helper.

- We want: one place to update paths if test folders move.
- So that: contract tests stop breaking due to tiny folder layout changes.

## Context (what exists today)

- There are multiple schema/contract tests that build relative paths like:
  - `../../src-tauri/gen/schemas/...`
  - `../../../../src-tauri/gen/schemas/...`
- Examples include:
  - `app/src/lib/settingsContract.test.ts`
  - `app/src/lib/contracts/schemas/{commandsSchemas,eventsSchemas,settingsSchemas}.test.ts`

## Acceptance criteria (how we know it’s done)

- [ ] Add a small helper module (suggested: `app/src/lib/contracts/contractTestPaths.ts`) that exports:
  - `resolveAppRoot()` (or equivalent)
  - `resolveSchemasDir()` (or equivalent)
  - and optionally `schemaPath(name: string)`.
- [ ] Update at least 2 existing contract test files to use the helper instead of duplicating relative `new URL(...)` / string path math.
- [ ] Tests still pass and remain deterministic.

## Edge cases / gotchas

- Ensure it works on Windows paths (don’t assume `/`). Prefer `new URL()` + `fileURLToPath()` patterns.
- Keep the helper test-only (or at least not pulled into runtime bundles by accident).

## Non-goals (explicitly out of scope)

- No change to schema contents.
- No change to which schemas are validated.

## Notes / hints

- Keep it minimal: one helper file + a few call-site updates.
