# Ticket: Split PromptSettings.tsx (extract Quick Replace panel)

## Goal (what we want)

Reduce churn and hook complexity in the giant settings UI by extracting the Quick Replace section from `app/src/components/settings/PromptSettings.tsx` into a smaller component (and hook if needed), without changing behavior.

- We want: smaller, single-purpose modules that are easier to test and maintain.
- So that: future prompt/router/Quick Ask/Quick Replace changes don’t cause mega-diffs.

## Context (what exists today)

- Hot spot file: `app/src/components/settings/PromptSettings.tsx` (very large; UI + business logic mixed).
- Quick Replace settings are part of the prompt settings surface and are wired into the Tauri settings layer.

## Acceptance criteria (how we know it’s done)

- [ ] Create a new component for the Quick Replace section (suggested: `app/src/components/settings/QuickReplaceSettings.tsx`).
- [ ] Move the JSX + local helper logic for Quick Replace out of `PromptSettings.tsx` into the new file.
- [ ] If there is stateful wiring logic (query/mutation/derived state), extract a small hook (suggested: `useQuickReplaceSettings.ts`) to keep the component mostly presentational.
- [ ] Keep UI behavior unchanged (same controls, same settings keys, same validation).
- [ ] Ensure any setting updates that affect runtime behavior still:
  - persist to the store, and
  - call `configAPI.syncPipelineConfig()` when required.

## Edge cases / gotchas

- Avoid breaking hook ordering (don’t conditionally call hooks during extraction).
- Make sure event emission (`settings-changed`) still happens where it did before.
- Watch for circular imports between settings components.

## Non-goals (explicitly out of scope)

- No redesign of the Quick Replace UX.
- No reformatting of unrelated parts of `PromptSettings.tsx`.

## Notes / hints

- This is intentionally “one panel only”, not a full split of the whole file.
