# High-impact refactors

These are high-leverage refactors worth doing soon, because they reduce bug risk and prevent “fix it in 3 places” drift.

- **Consolidate hotkey update hooks in `app/src/lib/queries.ts`.**
- **Shared TS entrypoint bootstrap (providers + root mounting).**
	- Evidence: multiple TS entrypoints bootstrap React similarly (e.g. `main`, `overlay`, `quick-ask`, `overlay-hover`).
	- Direction: `renderRoot(kind, <App />)` + shared `getRootElementOrThrow()`.
	- Why high: easy drift across `main`, `overlay`, `quick-ask`, `overlay-hover`.

- **Rust overlay window builder preset helper.**
	- Evidence: overlay/window builder chains tend to be copied with small tweaks (easy to drift across OS-specific rules).
	- Direction: `fn overlay_window_builder(...) -> WebviewWindowBuilder` with shared chain.
	- Why high: OS-specific window policy is subtle; multiple copies drift.

- **Centralize boot localStorage reads (accent + guide state).**
	- Evidence: boot-time localStorage reads exist in multiple apps (e.g. accent color + guide state).
	- Direction: `app/src/lib/bootStorage.ts` with safe get/set helpers.
