# Ticket: Add Wiremock contract test for STT provider (Groq)

<!-- ralph:skip -->

> Deprecated (consolidated): Groq STT is now covered by `020-contract-test-stt-speechmatics.md` (STT batch 02).

## Goal (what we want)

Add deterministic contract coverage for Groq STT.

## Context (what exists today)

- Provider code: `app/src-tauri/src/stt/groq.rs`
- Tests: `app/src-tauri/src/tests/stt_integration_tests.rs`

## Acceptance criteria (how we know it’s done)

- [ ] Add one request-shape test (path/headers + one body detail).
- [ ] Add one error parsing test.
- [ ] No real network.

## Backpressure (must be green)

- `pnpm -C app check:ci`
