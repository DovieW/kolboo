# Ticket: Add Wiremock contract tests for STT providers (Batch)

## Goal (what we want)

Add deterministic contract coverage for a small batch of STT providers.

## Context (what exists today)

- Providers in scope (pick 2–3 max for this batch):
	- `app/src-tauri/src/stt/deepgram.rs`
	- `app/src-tauri/src/stt/elevenlabs.rs`
	- `app/src-tauri/src/stt/assemblyai.rs`
	- `app/src-tauri/src/stt/speechmatics.rs`
	- `app/src-tauri/src/stt/groq.rs`
	- `app/src-tauri/src/stt/fireworks.rs`
	- `app/src-tauri/src/stt/aquavoice.rs`
- Tests: `app/src-tauri/src/tests/stt_integration_tests.rs`

## Acceptance criteria (how we know it’s done)

- [ ] For each chosen provider: add one request-shape test that asserts a key detail (path/query/headers).
- [ ] For each chosen provider: if request is JSON, assert a field; if multipart, assert part names/content-types.
- [ ] For each chosen provider: add one error parsing test.
- [ ] No real network.
