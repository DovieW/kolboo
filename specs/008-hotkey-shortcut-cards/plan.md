# Implementation Plan: Hotkey Shortcut Cards

**Branch**: `008-hotkey-shortcut-cards` | **Date**: 2026-02-01 | **Spec**: /specs/008-hotkey-shortcut-cards/spec.md
**Input**: Feature specification from `/specs/008-hotkey-shortcut-cards/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.github/agents/speckit.plan.agent.md` for the execution workflow.

## Summary

Replace the hotkeys settings list with a card-based UI that shows only configured shortcuts, supports adding new shortcut cards from a dropdown (including duplicates of the same action), and allows setting, unsetting, or deleting a card. Ensure key bindings remain unique across cards and persist through the existing settings storage.

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: TypeScript (strict) + Rust (Tauri)
**Primary Dependencies**: React/Vite, Tauri, TanStack Query
**Storage**: Tauri store (`settings.json`)
**Testing**: Vitest (`pnpm -C app test`), Rust tests (`pnpm -C app cargo:test`), CI gate (`pnpm -C app check:ci`)
**Target Platform**: Windows desktop (primary), macOS/Linux (secondary)
**Project Type**: Desktop app (Tauri)
**Performance Goals**: Keep the hotkeys page responsive and render shortcut cards without noticeable lag for typical shortcut counts (<50).
**Constraints**: Offline-capable; no network calls for settings; maintain settings migrations and immediate apply semantics.
**Scale/Scope**: Single-user local settings; dozens of shortcut cards per user.

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
specs/008-hotkey-shortcut-cards/
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

**Structure Decision**: UI changes will live under `app/src/**` and any settings normalization or backend updates will live under `app/src-tauri/src/**`. Feature documentation is isolated in `specs/008-hotkey-shortcut-cards/`.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No constitution violations are required for this plan.

## Phase 0: Research

- Create `research.md` to document decisions about performance expectations, constraints, and scope.

## Phase 1: Design & Contracts

- Create `data-model.md` for shortcut cards, types, and key bindings.
- Create OpenAPI contract in `contracts/` describing shortcut card CRUD.
- Create `quickstart.md` for validation steps and test commands.

## Phase 1: Agent Context Update

- Run `.specify/scripts/powershell/update-agent-context.ps1 -AgentType copilot` to refresh agent context.

## Constitution Check (Post-Design)

All checks remain satisfied after design artifacts are generated.
