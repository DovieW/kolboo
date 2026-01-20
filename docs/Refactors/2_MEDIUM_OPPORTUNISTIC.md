# Medium-priority refactors (opportunistic)

They tend to improve maintainability and reduce future friction, but they're not as directly "risk reducing" as the high-impact list.

- **Run Rust tests in CI.**
	- If CI runs clippy but not `cargo test`, regressions can slip through even when you have good tests locally.
	- Prefer a targeted run (only when Rust changes) if CI time is a concern.

- **Generate TypeScript types from Rust-exported JSON schemas.**
	- You already export schemas; generating TS types from them reduces manual sync work.
	- Keep the generated file isolated (e.g., `generated-types.ts`) so it’s clear what is safe to edit.

- **Mock providers for deterministic testing (STT/LLM/Embeddings).**
	- Many providers already support client injection.
	- Add “mock provider” implementations that return canned responses (and optionally simulate latency/errors).

- **Audio capture abstraction clean-up (test + portability).**
	- Wrap CPAL behind a trait so tests can use a mock capture source.
	- Bonus: makes device enumeration and fallback behavior easier to test.

- **Adopt structured tracing (better debugging and correlation).**
	- Add request/session spans so logs for “one recording” can be followed across async tasks.
	- This is especially useful once you have cancellation, retries, and concurrent windows/events.

- **Supply chain & security gates.**
	- Add Rust advisory/license checks (e.g., audit/deny style tooling) once and let them guard the repo.
	- Expect a one-time tuning cost.

- **Benchmarks for hot paths (optional but useful).**
	- Audio processing, VAD/resampling, routing similarity, and request-building are good candidates.
	- Benchmarks help prevent accidental slowdowns during refactors.
