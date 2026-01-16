# Ticket: Add Wiremock contract test for STT provider (AssemblyAI)


> Deprecated (consolidated): AssemblyAI STT is now covered by `017-contract-test-stt-deepgram.md` (STT batch 01).

## Goal (what we want)

Add deterministic contract coverage for AssemblyAI STT.

## Context (what exists today)

- Provider code: `app/src-tauri/src/stt/assemblyai.rs`
- Tests: `app/src-tauri/src/tests/stt_integration_tests.rs`

## Acceptance criteria (how we know it’s done)

- [ ] Add one request-shape test (path + header + one body detail).
- [ ] Add one error parsing test.
- [ ] No real network.

## Backpressure (must be green)

- `pnpm -C app check:ci`
