# Implementation Plan: Phase 2 Cloud Policy Packs

**Branch**: `012-cloud-policy-packs` | **Date**: 2026-02-14 | **Spec**: `specs/012-cloud-policy-packs/spec.md`
**Input**: Feature specification from `/specs/012-cloud-policy-packs/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.github/agents/speckit.plan.agent.md` for the execution workflow.

## Summary

Deliver cloud-backed enterprise policy packs that are safely fetched, verified, cached, applied to runtime settings, and clearly surfaced in the desktop UI. This phase prioritizes deterministic policy enforcement and diagnostics while preserving OSS baseline behavior if cloud policy is unavailable.

## Technical Context

**Language/Version**: TypeScript (strict) + Rust (Tauri)
**Primary Dependencies**: React/Vite, Mantine, TanStack Query, Tauri command/event system, store-backed settings layer
**Storage**: Tauri store (`settings.json`) for persisted policy cache and policy metadata (`policy_state` + effective policy payload)
**Testing**: Vitest (`pnpm -C app test`), Rust tests (`pnpm -C app cargo:test`), mixed validation (`pnpm -C app test:all`), final CI gate (`pnpm -C app check:ci`)
**Target Platform**: Windows desktop (primary), macOS/Linux (secondary)
**Project Type**: Desktop app (Tauri)
**Performance Goals**: Apply policy updates within 30s for successful manual syncs (SC-002) and keep UI settings refresh responsive (<1s after policy application event)
**Constraints**: Login remains optional, deterministic tests only, no secret/content leakage in diagnostics, offline fallback to last valid policy with bounded validity
**Scale/Scope**: `kolboo` desktop policy consumption/enforcement/diagnostics only; excludes cloud admin authoring and managed inference proxy implementation

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

```text
app/
├── src/
│   ├── components/settings/            # Policy visibility/enforcement UX
│   └── lib/
│       ├── tauri/settings.ts           # normalization + policy-aware persistence
│       ├── tauri/policy.ts             # policy command wrappers
│       ├── tauri/types.ts              # PolicyState and related types
│       └── queries.ts                  # policy fetch/apply/diagnostics queries
└── src-tauri/src/
  ├── commands/policy.rs              # Tauri policy sync/diagnostics commands
  ├── policy.rs                       # policy validation, application, cache logic
  └── lib.rs                          # command registration + startup wiring

specs/012-cloud-policy-packs/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
└── contracts/
```

**Structure Decision**: Keep policy trust/validation logic backend-owned (`src-tauri`) and expose typed, normalized policy state to the UI through Tauri wrappers and query hooks; enforce settings at the shared normalization layer to keep all windows and runtime behavior aligned.

## Complexity Tracking

No constitution violations requiring justification.

## Phase 0: Research

Completed in `specs/012-cloud-policy-packs/research.md`.

## Phase 1: Design & Contracts

- Data model: `specs/012-cloud-policy-packs/data-model.md`
- API contracts: `specs/012-cloud-policy-packs/contracts/policy-cloud-sync.yaml`
- Quickstart: `specs/012-cloud-policy-packs/quickstart.md`

## Constitution Check (Post-design)

- [x] Deterministic tests: no real network calls in tests; no real API keys required by default
- [x] UI↔backend contract: any command/event/type changes are updated in BOTH Rust and TypeScript
- [x] Settings discipline: any settings additions/changes include migrations/normalization and apply immediately at runtime
- [x] Secrets hygiene: no logging of secrets; redact sensitive data in logs
- [x] Tooling gate: plan includes how you’ll keep `pnpm -C app check:ci` green
