# Medium-priority refactors (opportunistic)

They tend to improve maintainability and reduce future friction, but they're not as directly "risk reducing" as the high-impact list.

- **Mock providers for deterministic testing (STT/LLM/Embeddings).**
	- Many providers already support client injection.
	- Add “mock provider” implementations that return canned responses (and optionally simulate latency/errors).

- **Audio capture abstraction clean-up (test + portability).**
	- Wrap CPAL behind a trait so tests can use a mock capture source.
	- Bonus: makes device enumeration and fallback behavior easier to test.

- **Adopt structured tracing (better debugging and correlation).**
	- Add request/session spans so logs for “one recording” can be followed across async tasks.
	- This is especially useful once you have cancellation, retries, and concurrent windows/events.


- **Benchmarks for hot paths (optional but useful).**
	- Audio processing, VAD/resampling, routing similarity, and request-building are good candidates.
	- Benchmarks help prevent accidental slowdowns during refactors.
