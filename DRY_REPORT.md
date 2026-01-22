# DRY_REPORT.md — Duplicate / Repeated Logic Report (Kolboo)

This file is intentionally **a living report**, not a one-off “generated dump”.

If you (Dovie) want fresh numbers, run the scan described in `DRY_PLAN.md` and then paste the _few_ highest-signal findings here.

## What counts as a good DRY finding (for Kolboo)

We care about repeated logic that causes:

- **bug drift** (“fix it in 3 places”)
- **high-churn editing pain**
- **subtle platform behavior** (especially overlay/window code)

We do _not_ care about repetitive-but-clear UI markup or tests that intentionally mirror scenarios.

## How to gather evidence

Use Stage 1 in `DRY_PLAN.md` (jscpd token clone detection) to produce JSON under `docs/Refactors/.dry-scan/`.

When you add an entry below, include:

- which files
- why it’s risky / annoying
- the smallest safe extraction you can imagine

## High-signal candidates (starter checklist)

These are areas that have historically been DRY-heavy in Kolboo:

- `app/src/lib/**` (settings + Tauri wrappers)
- `app/src/components/settings/**` (form rows, tooltips, reset buttons)
- `app/src/overlay/**` (consistency across windows/entries)
- `app/src-tauri/src/**` (window builder chains; provider request builders)

## Findings

Add entries in this format:

### <short title>

- Files:
  - `...`
- Why it matters:
- Suggested refactor:
- Test plan:

## “Do not DRY” reminders

- Generated files (`*.generated.*`)
- Tests that mirror similar scenarios on purpose
- Tiny UI snippets where extraction would hide intent
