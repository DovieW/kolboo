# Ticket: Add one deterministic provider contract test

## Goal (what we want)

Add exactly one new deterministic provider contract test in Rust using the existing local mock server approach.

## Context (what exists today)

- Tests live under: `app/src-tauri/src/tests/*_integration_tests.rs`
- There is already a local mock server / request capture pattern in the repo; follow that pattern.
- This test must be deterministic and must not hit the real network.

## Acceptance criteria (how we know it's done)

- [ ] Add one focused test in `app/src-tauri/src/tests/*_integration_tests.rs`.
- [ ] The test must not hit the network (only local mock server).
- [ ] The test verifies at least one request-shape detail (pick one):
  - headers
  - URL/path
  - query params
  - JSON body field
  - multipart part name/filename/content-type
- [ ] Update provider code only if necessary to add a test seam (prefer minimal changes).

## Edge cases / gotchas

- Keep fixtures stable: no timestamps, no randomness, no locale-dependent formatting.
- If multipart is involved, make sure ordering assumptions won’t make the test flaky.

## Backpressure (must be green)

- `pnpm -C app check:ci`

## Non-goals (explicitly out of scope)

- No provider refactor beyond what’s needed to add the test.
- No new live API calls.

## Notes / hints

- Prefer asserting the *minimum* request details that prove correctness.
- Keep the test small and isolated.

