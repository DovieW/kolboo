---
applyTo: '**'
---

# Testing guidance for Kolboo (for Ralph)

Dovie runs tickets through Ralph. Your goal: add tests quickly, keep them deterministic, and use the repo’s existing commands/patterns so you don’t have to “research” every time.

## Which test commands to run (use these exact ones)

- For **TypeScript/React** unit tests:
	- `pnpm -C app test`
	- Coverage: `pnpm -C app coverage`
- For **Rust (Tauri backend)** tests:
	- `pnpm -C app cargo:test`
- For “everything that CI cares about” (preferred gate):
	- `pnpm -C app check:ci`

When iterating locally during a ticket, it’s okay to run the smallest thing that proves your change:

- If you only touched `app/src/**`, start with `pnpm -C app test`.
- If you only touched `app/src-tauri/**`, start with `pnpm -C app cargo:test`.
- If you touched both (or you’re unsure), run `pnpm -C app test:all`.

## Where tests should go

- TypeScript tests live next to code and typically use Vitest:
	- Example pattern: `app/src/lib/*.test.ts`
- Rust tests are either:
	- inline `mod tests { ... }` in the same `.rs` file for unit tests, or
	- in `app/src-tauri/src/tests/*.rs` for broader coverage.

## Determinism rules (super important for Dovie)

- Do NOT add tests that require real network calls.
- Do NOT add tests that require API keys by default.
	- In Rust, integration tests that need keys should be `#[ignore]` and documented with how to run them.
- Prefer pure unit tests with fake inputs.
- Avoid timing/flaky assertions. If you must test time behavior, control time (mock) rather than sleeping.

## Vitest conventions

- Use `vitest` APIs: `describe`, `it`, `expect`.
- Keep tests small and specific (1 behavior per test), but group related cases with `it.each([...])`.
- Prefer testing exported functions (public behavior), not private internals.
- If you need to isolate modules, use `vi.mock(...)` and reset state between tests.

## Coverage notes (avoid surprise failures)

- The repo has per-file coverage thresholds in `app/vite.config.ts` (example: `src/lib/tauri.ts`).
- If a ticket adds code paths in thresholded files, add/adjust tests so the threshold stays green.
- If you genuinely need to change a threshold, treat it as a deliberate decision and explain why in the ticket output.

## Rust test conventions

- Prefer fast unit tests; don’t spawn long-running threads or rely on audio devices.
- If you add a new test module, keep it compile-fast and use `cargo test` friendly patterns.
- If a test must be manual (keys/hardware), mark it `#[ignore]` and include a comment like:
	- `// Run with: cargo test -- --ignored`

## When a test fails

- Read the failing assertion and fix the code or the test (don’t just loosen the assertion).
- If the failure is due to environment constraints (keys/hardware), convert that test to ignored/manual and add a separate unit test that validates the pure logic.

## Minimal change policy

- Don’t introduce new test libraries unless absolutely necessary.
- Don’t reformat unrelated files; keep diffs tight for Dovie.
