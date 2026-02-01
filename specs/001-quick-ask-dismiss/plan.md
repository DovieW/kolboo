# Implementation Plan: Quick Ask dismiss options

**Branch**: `001-quick-ask-dismiss` | **Date**: 2026-01-31 | **Spec**: `specs/001-quick-ask-dismiss/spec.md`
**Input**: Feature specification from `/specs/001-quick-ask-dismiss/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.github/agents/speckit.plan.agent.md` for the execution workflow.

## Summary

Add a per-profile Quick Ask dismiss mode (Manual default, Auto option), persist it with existing settings defaults/migrations, and update the Quick Ask overlay to honor click-away behavior plus an inline X close button that doesn’t change overlay height.

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
**Performance Goals**: Overlay interactions feel instant; no layout shift when showing the X button.
**Constraints**: Dismiss mode applied immediately; overlay height unchanged by close control.
**Scale/Scope**: Single desktop app with per-profile settings.

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
specs/001-quick-ask-dismiss/
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
│   ├── OverlayApp.tsx
│   ├── overlay-main.tsx
│   ├── lib/tauri/       # settings + invoke wrappers
│   └── lib/queries.ts
├── src-tauri/src/      # Rust/Tauri backend
│   ├── lib.rs           # settings defaults/migrations
│   └── pipeline.rs      # overlay-related state (if needed)
└── tests/              # (if present) test helpers, fixtures, etc.

docs/
scripts/
```

**Structure Decision**: UI behavior lives in `app/src` (overlay UI + settings UI), while settings defaults/migrations and any backend config sync live in `app/src-tauri/src`. Settings normalization stays in `app/src/lib/tauri/settings.ts`.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No constitution violations.
