# Low-urgency refactors (polish / preference)

These are quality-of-life improvements. They’re totally valid, but they’re the easiest to defer because they usually don’t change correctness or reduce major risk.

- **Biome rule tuning / hygiene improvements.**
	- Consider enabling rules that catch common async footguns (like “floating promises”).
	- Add complexity rules carefully (they can be noisy); use them to guide refactors, not as a religion.

- **Module boundary linting (avoid spaghetti imports).**
	- Enforce clean module boundaries so features don’t start importing random siblings via deep relative paths.

- **Gradually raise coverage thresholds (only alongside tests).**
	- Keep thresholds aligned with actual stability needs (don’t raise for sport).
	- Treat thresholds as guardrails: if you touch a thresholded file, add tests for the new branches.

- **Doc/guide polish.**
	- When the contract/migrations approach is settled, add a short “How to add a new setting” checklist.
