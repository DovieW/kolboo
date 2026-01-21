# Medium-priority refactors

- **Extract reusable Settings UI building blocks.**
	- Evidence: large, repeated UI patterns exist in settings screens (tooltips, reset buttons, select rows, repeated “row” layouts).
	- Direction: extract the *smallest* obvious components first (tooltips, reset buttons, select rows), then decide if a more generic section component is worth it.
	- Risk: medium (UI regressions).

- **Factor common request-building in Rust provider modules (LLM/STT).**
	- Evidence: provider modules often build similar request payload shapes with small variations.
	- Direction: helper functions for shared payload parts; keep provider-specific differences local.

- **Adopt structured tracing (better debugging and correlation).**
	- Add request/session spans so logs for "one recording" can be followed across async tasks.
	- This is especially useful once you have cancellation, retries, and concurrent windows/events.

- **Benchmarks for hot paths.**
	- Audio processing, VAD/resampling, routing similarity, and request-building are good candidates.
	- Benchmarks help prevent accidental slowdowns during refactors.
