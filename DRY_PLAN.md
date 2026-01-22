# DRY_PLAN.md — Plan to Detect and Triage DRY Violations (Kolboo)

Hi Dovie — this is the _repo-specific_ version of the DRY plan. It’s designed to be reproducible, so future “why is this duplicated?” investigations can be re-run.

## Goals

- Find **repeated logic** (not just repeated text).
- Prioritize duplication that creates **bug risk** or **high-churn editing pain**.
- Produce actionable outputs (that are actually reproducible in this repo today):
  - a `jscpd` JSON report under `docs/Refactors/.dry-scan/`
  - a short, human-readable write-up (update `DRY_REPORT.md`)

## Repo inventory (Stage 0)

### Languages/frameworks

- UI: TypeScript + React + Vite (`app/src/**`)
- Backend: Rust + Tauri (`app/src-tauri/src/**`)

### High-signal areas (priority modules)

These are the areas where DRY wins are usually worth it in Kolboo:

- `app/src/lib/**`
  - Settings normalization, Tauri wrappers, shared utilities
- `app/src/components/**`
  - Settings screens contain repeated UI/validation logic
- `app/src/overlay/**`
  - Overlay behavior needs consistency across windows; drift is bug-prone
- `app/src-tauri/src/**`
  - Overlay window construction and payload emission
  - Provider request construction (LLM/STT)

### Ignore list (noise reduction)

- `**/node_modules/**`
- `**/dist/**`
- `**/build/**`
- `**/target/**`
- `**/coverage/**`
- `**/tmp/**`
- `**/.git/**`
- `**/*.generated.*` (generated TS/Rust and event/type outputs)

## Stage 1 — Fast duplicate signal (token/text)

### Tooling

- Use `jscpd` for token-based clone detection.
- Start with **min 70 tokens** (works well for TS and Rust in this repo).

### How to run (reproducible)

1. Generate jscpd report (writes into `docs/Refactors/.dry-scan/`):

- `pnpm dlx jscpd --silent --min-tokens 70 --reporters json --output docs/Refactors/.dry-scan --ignore "**/node_modules/**,**/dist/**,**/build/**,**/target/**,**/coverage/**,**/tmp/**" app/src app/src-tauri/src`

2. Optional: open the generated JSON and cherry-pick a handful of the highest-signal clusters into `DRY_REPORT.md`.

## Stage 2 — Structure-aware similarity (optional, future)

Because “same logic, different variable names” is common in TS UI code, a lightweight AST normalization pass for TypeScript _can_ help.

This is **not implemented in this repo yet**. If we add it later, the rough shape would be:

- Parse TS/TSX files with the TypeScript compiler.
- Normalize identifier/literal nodes to `<ID>`/`<LIT>`.
- Compute fingerprints and a coarse SimHash for near-duplicates.

## Stage 3 — Semantic validation + DRY worthiness

For each high-impact cluster:

1. Confirm the behavior is actually the same (inputs/outputs/side effects).
2. Identify variation points (constants, optional branches, UI labels, etc.).
3. Score the refactor using this rubric:

| Dimension                           | Score   |
| ----------------------------------- | ------- |
| Frequency (occurrences)             | 0–5     |
| Complexity (branches/length)        | 0–5     |
| Churn risk (how often files change) | 0–5     |
| Refactor safety (purity/tests)      | 0–5     |
| Side-effect/coupling penalty        | 0 to −5 |

## Stage 4 — Refactor proposals (patch-ready, not auto-merged)

For top candidates:

- Propose an abstraction type:
  - Extract function
  - Extract helper + parameterize
  - Strategy/policy object
  - Template/pipeline stages
- Specify file location + signature.
- Include a small, deterministic test plan.

## Stage 5 — Continuous enforcement (optional)

- Consider running `jscpd` (or this script) in CI but gating only **new duplication**.
- Prefer reporting + trend charts over hard blocking until the baseline is cleaned up.
