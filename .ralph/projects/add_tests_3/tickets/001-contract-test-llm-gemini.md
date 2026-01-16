# Ticket: Add Wiremock contract test for LLM provider (Gemini)

<!-- ralph:skip -->

> Deprecated (consolidated): Gemini is now covered by `008-contract-test-llm-anthropic.md` (LLM batch 01).

## Goal (what we want)

Add one deterministic contract test for the Gemini LLM provider so request shape + error parsing can’t silently drift.

## Context (what exists today)

- Provider code: `app/src-tauri/src/llm/gemini.rs`
- Tests live under: `app/src-tauri/src/tests/llm_integration_tests.rs`

## Acceptance criteria (how we know it’s done)

- [ ] Add a local-mock-server test that asserts at least one of:
  - request URL/path
  - query params
  - JSON body shape
  - auth header
- [ ] Add an error parsing test for a representative Gemini error response.
- [ ] No real network calls.

## Edge cases / gotchas

- Gemini sometimes uses query params for API keys—avoid leaking secrets in logs/assertions.

## Backpressure (must be green)

- `pnpm -C app check:ci`
