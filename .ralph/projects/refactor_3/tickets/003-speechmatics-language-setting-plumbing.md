# Ticket: (Medium) Make Speechmatics language configurable (settings + plumbing)

## Goal (what we want)

Allow selecting Speechmatics language via a setting so it can be configured without code changes.

- We want: a persisted setting that controls Speechmatics language.
- So that: multilingual use cases work and we don’t have to hardcode language.

## Context (what exists today)

- There is an inline TODO in `app/src-tauri/src/stt/speechmatics.rs` about making language configurable.
- Repo conventions: adding a setting requires updating both:
  - Rust defaults/migrations (`ensure_default_settings(...)` / settings seeding)
  - TS normalization/migrations (`app/src/lib/tauri/settings.ts`)

## Acceptance criteria (how we know it’s done)

- [ ] Add a new setting key for Speechmatics language (name it clearly, e.g. `speechmatics_language`).
- [ ] Seed a safe default value in Rust settings defaults/migrations.
- [ ] Update TS settings types + normalization so the value is always a string (or defaulted).
- [ ] Plumb the setting into Speechmatics provider construction so requests use that language.
- [ ] Add a small unit test for any pure normalization helper you touch (TS or Rust), if feasible.
- [ ] No network calls in tests.

## Edge cases / gotchas

- Decide how to handle invalid values:
  - either clamp to default, or
  - accept any non-empty string and let the API validate.
- Don’t log the full settings blob if it contains secrets.

## Non-goals (explicitly out of scope)

- No UI dropdown (unless it’s already easy/obvious in the existing settings UI).
- No provider redesign.

## Notes / hints

- If the setting affects runtime behavior, remember the UI convention: persist to store _and_ call `configAPI.syncPipelineConfig()`.
