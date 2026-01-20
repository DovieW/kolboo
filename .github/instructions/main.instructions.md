---
applyTo: '**'
---

- Add tests when appropriate (but don't create unnecessary tests if there's such a thing).

- We have refactor docs in the Refactor folder. Whenever you are working on a task, and there is something that can be done to improve something you are working with, but it is out of scopre or too large of a change, then add it to the appropriate doc file.

- I have `pnpm dev` running an external terminal so I can see build errors already. Use VS Code Problems to make sure you didn't introduce real issues and fix all errors and warnings while you work.

- If your change overlaps with anything described in `.github/copilot-instructions.md` or `.github/instructions/**`, update those instruction files in the same PR so they stay accurate.

- When working on Ralph-created PRs, always confirm the PR head branch with `gh pr view` and check out that branch (it is usually `ralph/...`, not `pr/<id>`).

- Do not cut corners when making changes. When adding new things or fixing things, try to do it in a robust way.
  - Don't create massive files that do many different things.
