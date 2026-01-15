# Ticket: Add Wiremock contract tests for Embeddings providers

## Goal (what we want)

Add deterministic contract tests for embeddings providers so request shaping and error parsing don’t drift.

## Context (what exists today)

- Providers in scope (pick 1–3 max):
  - `app/src-tauri/src/embeddings/openai.rs`
  - `app/src-tauri/src/embeddings/cohere.rs`
  - `app/src-tauri/src/embeddings/fireworks.rs`
- Tests: add to an existing integration tests module or create a small new one under `app/src-tauri/src/tests/`.
- Wiremock pattern exists in Rust tests already.

## Acceptance criteria (how we know it’s done)

- [ ] For each chosen provider, add a request-shape test (request path + at least one JSON field).
- [ ] For each chosen provider, assert at least one header detail (auth header presence, content-type, etc).
- [ ] For each chosen provider, add one error parsing test for a non-2xx JSON error.
- [ ] No real network calls.

## Backpressure (must be green)

- `pnpm -C app check:ci`
