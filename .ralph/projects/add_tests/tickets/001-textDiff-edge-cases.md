# Ticket: Expand tests for textDiff edge cases

## Goal (what we want)

Increase coverage for `app/src/lib/textDiff.ts` with a few high-value edge cases so diff output stays correct and stable.

## Context (what exists today)

- Relevant code: `app/src/lib/textDiff.ts`
- Tests: Vitest (existing tests may already exist; extend rather than reinvent)

## Acceptance criteria (how we know it's done)

- [ ] Add/expand Vitest coverage for multiline differences.
- [ ] Add/expand coverage for whitespace-only changes (spaces/tabs).
- [ ] Add a unicode case that is stable cross-platform (e.g. emoji and/or combining marks) and asserts behavior without relying on platform-specific normalization.
- [ ] Cover empty string vs non-empty.
- [ ] Cover long-ish inputs (big enough to matter, but tests still run fast).

## Edge cases / gotchas

- CRLF vs LF newlines should not make tests flaky (normalize inputs inside the test if needed)
- Trailing newline vs no trailing newline
- Mixed tabs/spaces

## Backpressure (must be green)

- `pnpm -C app check:ci`

## Non-goals (explicitly out of scope)

- No “golden snapshot” files unless they clearly improve readability.
- No refactor of text diff algorithm unless a bug is found.

## Notes / hints

- Assertions should be specific (avoid huge snapshots).

