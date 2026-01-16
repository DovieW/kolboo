\# Ticket: Add Wiremock contract tests for STT providers (Batch 02)

## Goal (what we want)

Add deterministic contract coverage for a small batch of STT providers.

## Context (what exists today)

- Providers in scope (pick 2–4 max for this batch):
  - `app/src-tauri/src/stt/speechmatics.rs`
  - `app/src-tauri/src/stt/groq.rs`
  - `app/src-tauri/src/stt/fireworks.rs`
  - `app/src-tauri/src/stt/aquavoice.rs`
- Tests: `app/src-tauri/src/tests/stt_integration_tests.rs`

## Acceptance criteria (how we know it’s done)

- [ ] For each chosen provider: add one request-shape test (path/headers + one body detail).
- [ ] For each chosen provider: add one error parsing test.
- [ ] No real network.
