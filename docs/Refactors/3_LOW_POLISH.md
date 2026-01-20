# Low-urgency refactors (polish / preference)

These are quality-of-life improvements. They’re totally valid, but they’re the easiest to defer because they usually don’t change correctness or reduce major risk.

## A11y lint follow-ups

- **Review remaining a11y ignores in UI hotspots.**
	- Focus areas: `PromptSectionEditor` header action wrapper and the audio preview widgets in `AudioSettings`.
	- Goal: replace ignores with accessible patterns where feasible.

## Ralph harness (Copilot CLI) ergonomics

- **Remove hard-coded profile `ValidateSet` and discover profiles dynamically.**
	- Today the harness scripts list `kolboo` in a `ValidateSet`, which means adding a new profile requires editing scripts.
	- Follow-up idea: accept any `-Profile` string, then validate by checking for:
		- `ralph/<profile>/profile.json`, or legacy `ralph/profiles/<profile>.json`
	- Bonus: add a `List-Profiles` helper command/script.
