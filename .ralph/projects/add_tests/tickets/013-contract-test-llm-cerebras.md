# Ticket: Add Wiremock contract test for LLM provider (Cerebras)

<!-- ralph:skip -->

> Deprecated (consolidated): Cerebras is now covered by `011-contract-test-llm-cohere.md` (LLM batch 02).

## Goal (what we want)

Add deterministic contract coverage for the Cerebras LLM provider.

## Context (what exists today)

- Provider code: `app/src-tauri/src/llm/cerebras.rs`
- Tests: `app/src-tauri/src/tests/llm_integration_tests.rs`

## Acceptance criteria (how we know it’s done)

- [ ] Add one request-shape test (path + JSON field).
- [ ] Add one error parsing test.
- [ ] No real network.

## Backpressure (must be green)

- `pnpm -C app check:ci`
