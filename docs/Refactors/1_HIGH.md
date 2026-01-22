# High-impact refactors

These are high-leverage refactors worth doing soon, because they reduce bug risk and prevent “fix it in 3 places” drift.

- **Rust overlay window builder preset helper.**
- **Centralize boot localStorage reads (accent + guide state).**
	- Evidence: boot-time localStorage reads exist in multiple apps (e.g. accent color + guide state).
	- Direction: `app/src/lib/bootStorage.ts` with safe get/set helpers.
