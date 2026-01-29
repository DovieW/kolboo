# Implementation Plan: [FEATURE]

**Branch**: `[###-feature-name]` | **Date**: [DATE] | **Spec**: [link]
**Input**: Feature specification from `/specs/[###-feature-name]/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.github/agents/speckit.plan.agent.md` for the execution workflow.

## Summary

[Extract from feature spec: primary requirement + technical approach from research]

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: [e.g., TypeScript (strict) + Rust (Tauri) or NEEDS CLARIFICATION]
**Primary Dependencies**: [e.g., React/Vite, Tauri, TanStack Query or NEEDS CLARIFICATION]
**Storage**: [if applicable, e.g., Tauri store (settings.json), files, SQLite or N/A]
**Testing**: [e.g., Vitest (`pnpm -C app test`), Rust tests (`pnpm -C app cargo:test`) or NEEDS CLARIFICATION]
**Target Platform**: [e.g., Windows desktop (primary), macOS/Linux (secondary) or NEEDS CLARIFICATION]
**Project Type**: [desktop app (Tauri) - determines source structure]
**Performance Goals**: [domain-specific, e.g., 1000 req/s, 10k lines/sec, 60 fps or NEEDS CLARIFICATION]
**Constraints**: [domain-specific, e.g., <200ms p95, <100MB memory, offline-capable or NEEDS CLARIFICATION]
**Scale/Scope**: [domain-specific, e.g., 10k users, 1M LOC, 50 screens or NEEDS CLARIFICATION]

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [ ] Deterministic tests: no real network calls in tests; no real API keys required by default
- [ ] UI↔backend contract: any command/event/type changes are updated in BOTH Rust and TypeScript
- [ ] Settings discipline: any settings additions/changes include migrations/normalization and apply immediately at runtime
- [ ] Secrets hygiene: no logging of secrets; redact sensitive data in logs
- [ ] Tooling gate: plan includes how you’ll keep `pnpm -C app check:ci` green

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)
<!--
  ACTION REQUIRED: Replace the placeholder tree below with the concrete layout
  for this feature. Delete unused options and expand the chosen structure with
  real paths (e.g., apps/admin, packages/something). The delivered plan must
  not include Option labels.
-->

```text
app/
├── src/                # React/TypeScript UI
├── src-tauri/src/      # Rust/Tauri backend
└── tests/              # (if present) test helpers, fixtures, etc.

docs/
scripts/
```

**Structure Decision**: [Document the selected structure and reference the real
directories captured above]

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
