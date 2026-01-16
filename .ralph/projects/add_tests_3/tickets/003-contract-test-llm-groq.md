# Ticket: Add Wiremock contract test for LLM provider (Groq)


> Deprecated (consolidated): Groq is now covered by `008-contract-test-llm-anthropic.md` (LLM batch 01).

## Goal (what we want)

Add a deterministic contract test for the Groq LLM provider (request shape + error parsing).

## Context (what exists today)

- Provider code: `app/src-tauri/src/llm/groq.rs`
- Existing tests: `app/src-tauri/src/tests/llm_integration_tests.rs`

## Acceptance criteria (how we know it’s done)

- [ ] Contract test asserts at least one key request detail (path + JSON field and/or header).
- [ ] Error test asserts stable user-facing error extraction.
- [ ] Uses only Wiremock/local server.

## Backpressure (must be green)

- `pnpm -C app check:ci`
