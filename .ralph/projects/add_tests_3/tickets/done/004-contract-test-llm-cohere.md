# Ticket: Add Wiremock contract tests for LLM providers (Batch 02)

## Goal (what we want)

Add deterministic contract coverage for a small batch of LLM providers.

## Context (what exists today)

- Providers in scope (pick 2–3 max for this batch):
  - `app/src-tauri/src/llm/cohere.rs`
  - `app/src-tauri/src/llm/fireworks.rs`
  - `app/src-tauri/src/llm/cerebras.rs`
- Tests: `app/src-tauri/src/tests/llm_integration_tests.rs`

## Acceptance criteria (how we know it’s done)

- [ ] For each chosen provider, add a contract test that asserts request URL/path and at least one JSON field.
- [ ] For each chosen provider, add error parsing coverage for a representative error shape.
- [ ] No real network access.

