---
applyTo: '**'
---

- I have `pnpm dev` running an external terminal so I can see build errors already. Use VS Code Problems to make sure you didn't introduce real issues and fix all errors and warnings while you work.

- If your change overlaps with anything described in `.github/copilot-instructions.md` or `.github/instructions/**`, update those instruction files in the same PR so they stay accurate.

- When working on Ralph-created PRs, always confirm the PR head branch with `gh pr view` and check out that branch (it is usually `ralph/...`, not `pr/<id>`).
