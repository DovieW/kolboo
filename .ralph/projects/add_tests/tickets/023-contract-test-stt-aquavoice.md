# Ticket: Add Wiremock contract test for STT provider (AquaVoice)

<!-- ralph:skip -->

> Deprecated (consolidated): AquaVoice STT is now covered by `020-contract-test-stt-speechmatics.md` (STT batch 02).

## Goal (what we want)

Add deterministic contract coverage for AquaVoice STT.

## Context (what exists today)

- Provider code: `app/src-tauri/src/stt/aquavoice.rs`
- Tests: `app/src-tauri/src/tests/stt_integration_tests.rs`

## Acceptance criteria (how we know it’s done)

- [ ] Add one request-shape test (path/headers + one body detail).
- [ ] Add one error parsing test.
- [ ] No real network.
- [ ] If a base URL override exists (`aquavoice_base_url`), ensure the provider uses it in tests.

## Backpressure (must be green)

- `pnpm -C app check:ci`
