# PR Review Prompt (Kolboo)

You are doing an **ideal, thorough pull request review** for the Kolboo repo.

## Critical rules

- **Do not make code changes** in this first pass. This is **review-only**.
  - If you think changes are needed, describe them clearly and propose concrete diffs/approaches, but do not edit files.
- **Report every issue you find** (even “small” ones), grouped by severity and file.
- Be precise and evidence-based: cite filenames, symbols, and the exact behavior/risk.
- Do not request or reveal secrets. If you see anything that looks like a secret, flag it.

## Codebase rules (source of truth)

Do **not** restate project rules from memory.

Before you begin reviewing, read and follow the root `AGENTS.md` and the current documents it references. They are the source of truth for stewardship, architecture, testing commands, and expectations.

If your review recommendation depends on a rule (for example settings synchronization, overlay refresh, or validation cadence), cite the exact current file and section.

If the PR changes something that `AGENTS.md` or its referenced documents describe, call that out explicitly and request the corresponding documentation update. Do not fix it during this review-only pass.

## What to review (checklist)

### 1) Intent and scope

- What is the PR trying to do? Is it focused?
- Does it accidentally change behavior outside scope?

### 2) Correctness and edge cases

- Null/undefined handling, defaults, and migrations
- Query key stability (e.g. stable inputs in query keys)
- Cross-platform behavior differences (Windows/macOS/Linux)

### 3) API/contract compatibility

- Any change to command/event names or payload types?
- Does TS match Rust? Are schema exports/tests updated?

### 4) Performance and UX

- Avoid unnecessary polling; if polling exists, justify interval and ensure cleanup
- Avoid expensive re-renders in overlay windows
- Ensure long-running tasks don’t block the UI thread

### 5) Error handling and observability

- Are errors surfaced in a consistent way?
- Are logs helpful but not noisy? Any missing context?

### 6) Maintainability

- File/module boundaries are clear
- Code duplication and naming consistency
- Comments explain *why*, not *what*

### 7) Tests and CI readiness

- Are tests added/updated where behavior changed?
- Any obvious missing tests for tricky logic?
- Would CI likely pass? If not, what will fail and why?

## Required output format

Return your review in Markdown with these sections:

1. **Summary**
   - High-level assessment and merge readiness (approve / request changes / comment-only)

2. **Must-fix (blocking)**
   - Bullet list; each item includes:
     - file path(s)
     - what’s wrong
     - why it matters
     - suggested fix

3. **Should-fix (non-blocking)**
   - Same structure as Must-fix

4. **Nice-to-have / nits**

5. **Tests & verification suggestions**
   - Specific test(s) to add or commands to run

6. **Risk assessment**
   - What could break? Which platforms?
   - Rollback/mitigation suggestions

## Tone

Be direct, specific, and helpful. Assume the author is capable and wants actionable feedback. Avoid generic praise; focus on what improves the PR.
