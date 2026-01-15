# Ticket: Sync testing plan “Rolling TODO” with current status

## Goal (what we want)

Make sure `docs/Plans/TESTING_AND_QUALITY_PLAN.md` reflects what’s actually been done, so it stays trustworthy.

- We want: the plan’s checkboxes to match reality.
- So that: future you (hi Dovie) doesn’t re-do work or miss what’s next.

## Context (what exists today)

- Plan: `docs/Plans/TESTING_AND_QUALITY_PLAN.md`
- Some items are done in code/docs but may still be unchecked in the plan.

## Acceptance criteria (how we know it’s done)

- [ ] Update the Rolling TODO checkboxes to reflect reality.
- [ ] If we completed an item via a different doc (e.g. a seam audit note), add a short link/reference.
- [ ] Ensure the plan still has a clear list of what remains.

## Edge cases / gotchas

- Don’t rewrite the whole plan—only update status + tiny clarifying notes.

## Backpressure (must be green)

- `pnpm -C app check:ci`

## Non-goals (explicitly out of scope)

- Rewriting or restructuring the entire plan.
