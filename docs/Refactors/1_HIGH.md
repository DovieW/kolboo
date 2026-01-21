# High-impact refactors

These are high-leverage refactors worth doing soon, because they reduce bug risk and prevent “fix it in 3 places” drift.

- **Consolidate hotkey update hooks in `app/src/lib/queries.ts`.**
	- Evidence: `dry_clusters.json` cluster `ts-ast:exact:6e4f97...` (6 occurrences).
	- Direction: extract a small helper that parameterizes only the hotkey key + mutation wiring.
	- Why high: this is shared app logic (not just UI), and edits are likely to happen over time.

- **Shared TS entrypoint bootstrap (providers + root mounting).**
	- Evidence: manual audit in `docs/Refactors/REPEATED_CODE_AUDIT.md`.
	- Direction: `renderRoot(kind, <App />)` + shared `getRootElementOrThrow()`.
	- Why high: easy drift across `main`, `overlay`, `quick-ask`, `overlay-hover`.

- **Rust overlay window builder preset helper.**
	- Evidence: manual audit in `docs/Refactors/REPEATED_CODE_AUDIT.md`.
	- Direction: `fn overlay_window_builder(...) -> WebviewWindowBuilder` with shared chain.
	- Why high: OS-specific window policy is subtle; multiple copies drift.

- **Centralize boot localStorage reads (accent + guide state).**
	- Evidence: manual audit in `docs/Refactors/REPEATED_CODE_AUDIT.md`.
	- Direction: `app/src/lib/bootStorage.ts` with safe get/set helpers.
