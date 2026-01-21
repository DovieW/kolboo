# Medium-priority refactors

- **Extract reusable Settings UI building blocks.**
	- Evidence: Stage 1 clones (examples):
		- `jscpd:f2d616...` (large repeated blocks inside `ProvidersSettings.tsx`)
		- `jscpd:c9c312...` / `jscpd:836e56...` (repeats inside `QuickReplaceSettings.tsx`)
		- `jscpd:76f69f...` / `jscpd:d59f0c...` (repeats inside `RewriteSettingsSection.tsx`)
	- Direction: extract the *smallest* obvious components first (tooltips, reset buttons, select rows), then decide if a more generic section component is worth it.
	- Risk: medium (UI regressions).

- **Factor common request-building in Rust provider modules (LLM/STT).**
	- Evidence: `jscpd:0cf1c4...` inside `app/src-tauri/src/llm/gemini.rs`.
	- Direction: helper functions for shared payload parts; keep provider-specific differences local.

- **Adopt structured tracing (better debugging and correlation).**
	- Add request/session spans so logs for "one recording" can be followed across async tasks.
	- This is especially useful once you have cancellation, retries, and concurrent windows/events.

- **Benchmarks for hot paths.**
	- Audio processing, VAD/resampling, routing similarity, and request-building are good candidates.
	- Benchmarks help prevent accidental slowdowns during refactors.
