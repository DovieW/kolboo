# Medium-priority refactors

- **Mock providers for deterministic testing (STT/LLM/Embeddings).**
	- STT and LLM mock providers exist in `pipeline/tests.rs` with configurable latency and error simulation via `MockBehavior`.
	- Remaining: Add mock embeddings provider for full offline routing tests.

- **Adopt structured tracing (better debugging and correlation).**
	- Add request/session spans so logs for "one recording" can be followed across async tasks.
	- This is especially useful once you have cancellation, retries, and concurrent windows/events.

- **Benchmarks for hot paths.**
	- Audio processing, VAD/resampling, routing similarity, and request-building are good candidates.
	- Benchmarks help prevent accidental slowdowns during refactors.
