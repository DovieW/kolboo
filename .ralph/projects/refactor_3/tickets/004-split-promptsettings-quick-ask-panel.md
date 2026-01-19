# Ticket: (Medium) Split PromptSettings: extract Quick Ask panel

## Goal (what we want)

Reduce complexity in the settings UI by extracting the Quick Ask portion of `PromptSettings.tsx` into a smaller component (and hook helpers if needed).

- We want: smaller files with less tangled state.
- So that: future changes are less scary and hook dependency management is easier.

## Context (what exists today)

- Hot spot file: `app/src/components/settings/PromptSettings.tsx` (large; mixed concerns).
- There is a refactor note suggesting splitting into panels: presets editor, intent router panel, Quick Ask panel, Quick Replace panel.

## Acceptance criteria (how we know it’s done)

- [ ] Extract the Quick Ask panel UI into a new component (suggested path: `app/src/components/settings/prompt/QuickAskPanel.tsx`).
- [ ] Keep external behavior identical:
  - same settings keys read/written
  - same validation/disabled states
  - no layout regressions
- [ ] Avoid drive-by reformatting; keep diffs tight.
- [ ] Typecheck passes.

## Edge cases / gotchas

- Be careful with React hook dependency warnings; avoid “fix by disabling” unless already consistent with repo standards.
- Don’t change settings semantics around `null` vs missing.

## Non-goals (explicitly out of scope)

- No redesign of the Quick Ask feature.
- No styling refresh.

## Notes / hints

- If the extracted component needs lots of props, consider a tiny `useQuickAskSettings()` helper to keep it readable.
