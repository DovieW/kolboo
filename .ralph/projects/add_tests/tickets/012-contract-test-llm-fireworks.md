# Ticket: Add Wiremock contract test for LLM provider (Fireworks)

<!-- ralph:skip -->

> Deprecated (consolidated): Fireworks is now covered by `011-contract-test-llm-cohere.md` (LLM batch 02).

## Goal (what we want)

Add deterministic contract coverage for the Fireworks LLM provider.

## Context (what exists today)

- Provider code: `app/src-tauri/src/llm/fireworks.rs`
- Tests: `app/src-tauri/src/tests/llm_integration_tests.rs`

## Acceptance criteria (how we know it’s done)

- [ ] Add one contract test that asserts request path + at least one JSON body field.
- [ ] Add one error parsing test.
- [ ] Use local mock server only.

## Backpressure (must be green)

- `pnpm -C app check:ci`
