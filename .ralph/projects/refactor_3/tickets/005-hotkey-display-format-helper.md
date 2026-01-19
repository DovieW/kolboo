# Ticket: (Low) Add a display formatter for hotkeys (modifiers first)

## Goal (what we want)

Improve hotkey readability for humans without changing the canonical serialized shortcut string.

- We want: display strings like `Control+A` (modifiers first).
- So that: settings UI looks more natural while persistence remains stable.

## Context (what exists today)

- There’s a refactor note about hotkey normalization output ordering.
- Current normalization sorts tokens alphabetically (canonical strings like `a+control`).

## Acceptance criteria (how we know it’s done)

- [ ] Keep the existing canonical/normalized representation unchanged (no settings migration).
- [ ] Add a new helper function for display formatting (e.g. `formatShortcutForDisplay(...)`).
- [ ] Update the UI to use the display formatter where shortcuts are shown to users.
- [ ] Add unit tests for the display formatter:
  - modifiers come first
  - stable casing (whatever the app convention is)
  - handles unknown tokens reasonably

## Edge cases / gotchas

- Avoid breaking any parsing logic that expects the canonical form.
- If there are platform differences (Cmd vs Ctrl), keep the behavior explicit and tested.

## Non-goals (explicitly out of scope)

- No change to how shortcuts are stored.
- No broad UX overhaul.

## Notes / hints

- This is intentionally “small polish”: new helper + tests + minimal call-site change.
