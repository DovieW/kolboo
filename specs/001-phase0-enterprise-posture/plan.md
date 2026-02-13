# Implementation Plan: Phase 0 Enterprise Posture

**Branch**: `001-phase0-enterprise-posture` | **Date**: 2026-02-13 | **Spec**: `specs/001-phase0-enterprise-posture/spec.md`
**Input**: Feature specification from `/specs/001-phase0-enterprise-posture/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.github/agents/speckit.plan.agent.md` for the execution workflow.

## Summary

Implement a Phase 0 enterprise policy posture for Kolboo: enforce policy-constrained settings, expose a user-facing policy status screen, and provide redacted diagnostics export, while keeping the app fully usable without login or managed inference. Enforcement is local-first and deterministic, using the existing settings normalization/store flow plus runtime sync/event propagation.

## Technical Context

**Language/Version**: TypeScript (strict) + Rust (Tauri)
**Primary Dependencies**: React/Vite, TanStack Query, Mantine UI, Tauri, `@tauri-apps/plugin-store`
**Storage**: Tauri store (`settings.json`) + local policy state/cache metadata
**Testing**: Vitest (`pnpm -C app test`), Rust tests (`pnpm -C app cargo:test`), CI gate (`pnpm -C app check:ci`)
**Target Platform**: Windows desktop (primary), macOS/Linux (secondary)
**Project Type**: Desktop app (Tauri)
**Performance Goals**: No perceptible settings-screen lag; policy apply/refresh should feel immediate in UI
**Constraints**: Offline-capable, no account required in Phase 0, deterministic behavior, no secrets in diagnostics
**Scale/Scope**: Phase 0 only (policy posture + transparency + diagnostics), no billing/auth/managed inference implementation

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
│   ├── components/settings/
│   └── lib/
│       ├── queries.ts
│       └── tauri/
│           ├── settings.ts
│           ├── types.ts
│           └── events.ts
└── src-tauri/src/
  ├── lib.rs
  ├── settings/
  ├── settings.rs
  ├── bootstrap/
  └── event_payloads.rs
```

**Structure Decision**: Keep policy normalization and enforcement logic centered in `app/src/lib/tauri/settings.ts` with UI affordances under `app/src/components/settings`. Backend remains source of runtime sync/event emission and policy state serialization.

## Complexity Tracking

No constitution violations requiring justification.

## Phase 0: Research

Completed in `specs/001-phase0-enterprise-posture/research.md`.

## Phase 1: Design & Contracts

- Data model: `specs/001-phase0-enterprise-posture/data-model.md`
- API contracts: `specs/001-phase0-enterprise-posture/contracts/policy-local-api.yaml`
- Quickstart: `specs/001-phase0-enterprise-posture/quickstart.md`

## Constitution Check (Post-design)

- [x] Deterministic tests: no real network calls in tests; no real API keys required by default
- [x] UI↔backend contract: any command/event/type changes are updated in BOTH Rust and TypeScript
- [x] Settings discipline: any settings additions/changes include migrations/normalization and apply immediately at runtime
- [x] Secrets hygiene: no logging of secrets; redact sensitive data in logs
- [x] Tooling gate: plan includes how you’ll keep `pnpm -C app check:ci` green
