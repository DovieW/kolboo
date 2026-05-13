---
applyTo: '**'
---

## Which test commands to run (use these exact ones)

### Rust cache setup for local runs

- Before running any command below that invokes Cargo (`pnpm -C app cargo:test`, `pnpm -C app test:all`, `pnpm -C app check:ci`), set `RUSTC_WRAPPER=sccache` in the current shell when `sccache` is available.
- If `sccache` is not installed, clear `RUSTC_WRAPPER` so commands run with plain `rustc`.
- Also set `CARGO_BUILD_JOBS` to a conservative local value (recommended: ~half logical cores, capped at 8) so test/check runs do not monopolize the machine.

- For **TypeScript/React** unit tests:
	- `pnpm -C app test`
	- Coverage: `pnpm -C app coverage`
- For **Rust (Tauri backend)** tests:
	- `pnpm -C app cargo:test`
- For **Rust dead-code / unused dependency checks**:
	- `pnpm -C app cargo:deadcode`
	- `pnpm -C app cargo:deaddeps` (requires `cargo install cargo-machete --locked`; GitHub Rust CI installs it explicitly)
- For “everything that CI cares about” (preferred gate):
	- `pnpm -C app check:ci`
	- Note: GitHub Rust CI also runs `cargo machete app/src-tauri` for unused Cargo dependencies.

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

- The repo has per-file coverage thresholds in `app/vite.config.ts` (example: `src/lib/tauri/commands.ts`, and other files under `src/lib/tauri/**`).
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
