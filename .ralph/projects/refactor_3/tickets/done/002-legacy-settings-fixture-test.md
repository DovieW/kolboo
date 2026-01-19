# Ticket: (Medium) Add a real legacy settings fixture test

## Goal (what we want)

Increase confidence in settings migrations/normalization by adding at least one realistic “older settings.json shape” fixture test.

- We want: a deterministic test that proves we don’t break older users’ configs.
- So that: future settings refactors don’t silently regress.

## Context (what exists today)

- Settings normalization/migrations live in the frontend settings layer (see `app/src/lib/tauri/settings.ts`).
- We have a refactor note that we should add at least one legacy fixture test.
- This should be **pure and deterministic** (no network, no API keys).

## Acceptance criteria (how we know it’s done)

- [ ] Create a test file near the settings layer (example: `app/src/lib/tauri/settings.legacy.test.ts`).
- [ ] Add at least one JSON fixture representing an older/partial/invalid `settings.json` shape.
  - Fixture can be inline in the test, or in `app/src/lib/tauri/__fixtures__/...json`.
- [ ] The test must call the real normalization/migration entrypoint(s) and assert:
  - missing keys are defaulted correctly
  - invalid values are coerced or rejected as intended
  - `null` semantics (“explicitly disabled”) are preserved where applicable
- [ ] Test runs locally and in CI (no environment dependencies).

## Edge cases / gotchas

- Don’t snapshot huge objects; assert a small set of key fields that prove the migration worked.
- Prefer one fixture that matches something we’ve actually seen drift on (prompt profiles, quick replace, router settings, etc.).

## Non-goals (explicitly out of scope)

- No new settings UI.
- No new setting keys unless required to make the test meaningful.

## Notes / hints

- Keep it boring: fake input -> normalize -> assert.
