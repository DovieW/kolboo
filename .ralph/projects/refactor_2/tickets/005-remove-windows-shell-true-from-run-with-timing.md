# Ticket: Avoid `shell: true` in run-with-timing (Windows)

## Goal (what we want)

Remove the Windows `shell: true` execution path from `app/scripts/run-with-timing.mjs` (or tighten it) so we avoid Node’s DEP0190 warning and reduce command-injection footguns.

- We want: `shell: false` execution on Windows where possible.
- So that: local dev output is cleaner and the runner is safer.

## Context (what exists today)

- There’s a known DX issue: on Windows the script often runs child processes with `shell: true`, which triggers a Node warning (DEP0190) and is generally less safe.
- This is tracked as a refactor idea in `docs/REFACTOR_TODO.md`.

## Acceptance criteria (how we know it’s done)

- [ ] Update `app/scripts/run-with-timing.mjs` so Windows does not require `shell: true` for normal usage.
  - Suggested approach: explicitly resolve `node_modules/.bin/<cmd>` and pass it as the executable, keeping args as an array.
- [ ] Keep behavior the same:
  - commands still run
  - timing output format remains compatible
- [ ] If there are edge cases where `shell: true` is still required, constrain it to an explicit opt-in flag (documented in the script header).

## Edge cases / gotchas

- `.cmd` shims on Windows vs real JS bin scripts.
- Quoting: do not rebuild a single command string.

## Non-goals (explicitly out of scope)

- No new dependency unless absolutely necessary.
- No large rewrite of the timing script.

## Notes / hints

- A tiny helper like `resolveBin("pnpm")` can be enough.
