# Low-urgency refactors (polish / preference)

These are quality-of-life improvements. They’re totally valid, but they’re the easiest to defer because they usually don’t change correctness or reduce major risk.

## Hotkey normalization UX

- **Decide whether `normalize_shortcut_string(...)` should output “modifiers first”.**
	- Current behavior sorts tokens alphabetically (e.g. `"a+control"`).
	- Options:
		- Keep current behavior and format shortcuts for display in the UI, or
		- Change normalization to sort modifiers before non-modifiers (and update persisted settings/tests).

## A11y lint follow-ups

- **Audit and reduce inline Biome a11y ignores.**
	- Some patterns are genuinely constrained (e.g. Mantine `Accordion.Control` nesting rules), but others can be fixed with small UI refactors.
	- Goal: reduce “ignore sprawl” and re-evaluate which a11y rules can be safely re-enabled.

## Ralph harness (Copilot CLI) ergonomics

- **Remove hard-coded profile `ValidateSet` and discover profiles dynamically.**
	- Today the harness scripts list `kolboo` in a `ValidateSet`, which means adding a new profile requires editing scripts.
	- Follow-up idea: accept any `-Profile` string, then validate by checking for:
		- `ralph/<profile>/profile.json`, or legacy `ralph/profiles/<profile>.json`
	- Bonus: add a `List-Profiles` helper command/script.
