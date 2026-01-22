# Medium-priority refactors

- **Adopt structured tracing (better debugging and correlation).**
	- Add request/session spans so logs for "one recording" can be followed across async tasks.
	- This is especially useful once you have cancellation, retries, and concurrent windows/events.

- **Benchmarks for hot paths.**
	- Audio processing, VAD/resampling, routing similarity, and request-building are good candidates.
	- Benchmarks help prevent accidental slowdowns during refactors.
