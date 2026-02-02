# Implementation Plan: Paste Safety Toggle

**Branch**: `001-paste-smart-toggle` | **Date**: 2026-02-01 | **Spec**: `specs/001-paste-smart-toggle/spec.md`
**Input**: Feature specification from `/specs/001-paste-smart-toggle/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.github/agents/speckit.plan.agent.md` for the execution workflow.

## Summary

Add a UI-tab setting (default off) that lets users enable/disable smart paste protection. When enabled, keep the existing Windows safety checks that block insertion into sensitive targets; when disabled, bypass the safety checks and attempt normal output. Persist the new setting in `settings.json` and apply changes immediately.

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: TypeScript (strict) + Rust (Tauri)
**Primary Dependencies**: React/Vite, Mantine UI, TanStack Query, Tauri + @tauri-apps/plugin-store
**Storage**: Tauri store (`settings.json`)
**Testing**: Vitest (`pnpm -C app test`), Rust tests (`pnpm -C app cargo:test`)
**Target Platform**: Windows desktop (primary), macOS/Linux (secondary)
**Project Type**: Desktop app (Tauri)
**Performance Goals**: No new performance goals; keep output latency consistent with current behavior
**Constraints**: Settings changes must apply immediately without restart
**Scale/Scope**: Single new global setting + Windows output safety toggle

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] Deterministic tests: no real network calls in tests; no real API keys required by default
- [x] UI↔backend contract: any command/event/type changes are updated in BOTH Rust and TypeScript
- [x] Settings discipline: any settings additions/changes include migrations/normalization and apply immediately at runtime
- [x] Secrets hygiene: no logging of secrets; redact sensitive data in logs
- [x] Tooling gate: plan includes how you’ll keep `pnpm -C app check:ci` green

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
├── src/
│   ├── components/settings/UiSettings.tsx
│   └── lib/
│       ├── queries.ts
│       └── tauri/
│           ├── settings.ts
│           └── types.ts
├── src-tauri/src/
│   ├── lib.rs
│   ├── settings/defaults.rs
│   └── windows_uia/
│       ├── insert.rs
│       └── safety.rs
```

**Structure Decision**: Settings changes live in `app/src/lib/tauri` and UI controls in `app/src/components/settings`. Windows output safety behavior is handled in `app/src-tauri/src/windows_uia` and `app/src-tauri/src/lib.rs`.

## Complexity Tracking

No constitution violations requiring justification.

## Phase 0: Research

Completed in `specs/001-paste-smart-toggle/research.md`.

## Phase 1: Design & Contracts

- Data model: `specs/001-paste-smart-toggle/data-model.md`
- API contract: `specs/001-paste-smart-toggle/contracts/settings.patch.yaml`
- Quickstart: `specs/001-paste-smart-toggle/quickstart.md`

## Constitution Check (Post-design)

- [x] Deterministic tests: no real network calls in tests; no real API keys required by default
- [x] UI↔backend contract: any command/event/type changes are updated in BOTH Rust and TypeScript
- [x] Settings discipline: any settings additions/changes include migrations/normalization and apply immediately at runtime
- [x] Secrets hygiene: no logging of secrets; redact sensitive data in logs
- [x] Tooling gate: plan includes how you’ll keep `pnpm -C app check:ci` green
