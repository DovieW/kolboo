# Ticket: Add Wiremock contract tests for LLM providers (Batch)

## Goal (what we want)

Add deterministic contract tests that lock down request/response handling for a small batch of LLM providers.

## Context (what exists today)

- Providers in scope (pick 2–3 max for this batch):
  - `app/src-tauri/src/llm/anthropic.rs`
  - `app/src-tauri/src/llm/gemini.rs`
  - `app/src-tauri/src/llm/groq.rs`
  - `app/src-tauri/src/llm/cohere.rs`
  - `app/src-tauri/src/llm/fireworks.rs`
  - `app/src-tauri/src/llm/cerebras.rs`
- Existing contract tests pattern: `app/src-tauri/src/tests/llm_integration_tests.rs`
- Local mock server approach (Wiremock) is already used for other providers.

## Acceptance criteria (how we know it’s done)

- [ ] For each chosen provider, add at least one test that asserts a specific request-shape detail (URL/path + a JSON field and/or header).
- [ ] For each chosen provider, add at least one test for error parsing (non-2xx response -> surfaced error message is stable).
- [ ] Tests use only local mock server; no real network.
- [ ] If needed, add a minimal base URL override to a provider (defaulting to production).

## Edge cases / gotchas

- Avoid asserting too much (don’t lock down irrelevant fields that change often).
