# Ticket: Add Wiremock contract test for Embeddings provider (Cohere)


> Deprecated (consolidated): Cohere embeddings are now covered by `014-contract-test-embeddings-openai.md`.

## Goal (what we want)

Add deterministic contract coverage for Cohere embeddings.

## Context (what exists today)

- Provider code: `app/src-tauri/src/embeddings/cohere.rs`
- Wiremock tests pattern exists under `app/src-tauri/src/tests/`.

## Acceptance criteria (how we know it’s done)

- [ ] Add one request-shape test (path + JSON field(s)).
- [ ] Add one error parsing test.
- [ ] No real network.

## Backpressure (must be green)

- `pnpm -C app check:ci`
