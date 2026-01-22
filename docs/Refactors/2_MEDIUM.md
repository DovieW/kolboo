# Medium-priority refactors

## Extract `stop_recording` out of `lib.rs` (optional)

- **Where:** `app/src-tauri/src/lib.rs` (`stop_recording`)
- **Why:** it’s a very large function that mixes a bunch of concerns (audio cues, pipeline stop/transcribe, quick-ask, quick-replace, output, history/logs, overlay visibility). That makes it hard to navigate and easy to cause merge conflicts.
- **When to do it:** only if we’re already modifying that flow, or if `lib.rs` readability is actively slowing us down.
- **Low-risk approach:**
	- Create `app/src-tauri/src/recording_stop.rs` with `pub(crate) fn stop_recording(...)`.
	- In `lib.rs`, keep the same `#[cfg(desktop)] pub(crate) fn stop_recording(...)` signature but make it a tiny wrapper that calls into the module (or re-export it).
	- No behavior changes; just move code.
	- Run Rust format + a quick compile check.

