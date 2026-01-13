# Ralph mini-ticket template (Option A)

Use this for *small*, deterministic work items (tests, bugfixes, refactors) that should complete in one loop iteration.

## Title

Short, specific, action-oriented.

Example: `Add tests for formatError() unknown shapes`

## Goal (what we want)

One or two sentences in plain language.

- We want: …
- So that: …

## Context (what exists today)

Bullet points with only the info needed to do the work.

- Relevant files: `path/to/file.ts`, `path/to/file.test.ts`
- Current behavior: …
- Constraints: …

## Acceptance criteria (how we know it’s done)

Write this like “Given/When/Then” or a checklist. Keep it **measurable**.

- [ ] Given … when … then …
- [ ] Given … when … then …

## Edge cases / gotchas

List the tricky cases that usually get missed.

- …
- …

## Backpressure (must be green)

This is the hard gate before the ticket is considered done.

- `pnpm -C app check:ci`

## Non-goals (explicitly out of scope)

Optional, but helpful to prevent “helpful” scope creep.

- Not doing: …

## Notes / hints

Optional.

- If you need to touch settings that affect runtime behavior: persist to store *and* call `configAPI.syncPipelineConfig()`.
- If overlays depend on the change: emit `settings-changed`.
