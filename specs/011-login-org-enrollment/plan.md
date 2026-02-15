# Implementation Plan: Phase 1 Login and Org Enrollment

**Branch**: `011-login-org-enrollment` | **Date**: 2026-02-13 | **Spec**: `specs/011-login-org-enrollment/spec.md`
**Input**: Feature specification from `/specs/011-login-org-enrollment/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.github/agents/speckit.plan.agent.md` for the execution workflow.

## Summary

Deliver optional account login and entitlement/org-enrollment visibility while preserving full non-login usability. Implement a local-first `LicenseState` lifecycle in Tauri (secure token storage, refresh, offline grace), expose typed command wrappers and query hooks in UI, add account settings UX that clearly reflects signed-in/signed-out, org context, and grace/expired states, and include minimum viable Sentry instrumentation for account/licensing reliability telemetry with redaction-safe payloads.

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: TypeScript (strict) + Rust (Tauri)
**Primary Dependencies**: React/Vite, Mantine, TanStack Query, `@tauri-apps/api`, Tauri Rust command/event system, Sentry SDKs used by desktop surfaces
**Storage**: Tauri store (`settings.json`) for non-secret cached state + OS secure storage for auth/session tokens
**Testing**: Vitest (`pnpm -C app test`), Rust tests (`pnpm -C app cargo:test`), CI gate (`pnpm -C app check:ci`)
**Target Platform**: Windows desktop (primary), macOS/Linux (secondary)
**Project Type**: Desktop app (Tauri)
**Performance Goals**: Account state visible shortly after startup; sign-in flow supports SC-001 (95% complete in ≤60s)
**Constraints**: Login remains optional; deterministic tests only; 7-day offline grace behavior; no secret/token leakage; no user content in telemetry payloads
**Scale/Scope**: Phase 1 only (login, entitlement state, org context, grace handling, and minimum Sentry reliability telemetry); excludes managed inference routing and cloud policy distribution

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
│   │   ├── AccountSettings.tsx           # new
│   │   └── index.ts                      # updated export
│   └── lib/
│       ├── queries.ts                    # add account/license queries
│       └── tauri/
│           ├── commands.ts               # add license command wrappers
│           ├── license.ts                # new
│           └── types.ts                  # add LicenseState/Tier types
└── src-tauri/src/
  ├── licensing.rs                      # new module
  ├── lib.rs                            # register new commands/module
  └── commands/
    └── licensing.rs                  # new tauri command handlers

specs/011-login-org-enrollment/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
└── contracts/
```

**Structure Decision**: Keep all account state lifecycle logic in backend Rust (`licensing.rs` + command handlers) with typed UI wrappers under `app/src/lib/tauri/`. Expose user-facing behavior through a dedicated account settings section and query hooks, matching current settings architecture.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No constitution violations requiring justification.

## Phase 0: Research

Completed in `specs/011-login-org-enrollment/research.md`.

## Phase 1: Design & Contracts

- Data model: `specs/011-login-org-enrollment/data-model.md`
- API contracts: `specs/011-login-org-enrollment/contracts/license-local-api.yaml`
- Quickstart: `specs/011-login-org-enrollment/quickstart.md`
- Task checklist: `specs/011-login-org-enrollment/tasks.md`

## Constitution Check (Post-design)

- [x] Deterministic tests: no real network calls in tests; no real API keys required by default
- [x] UI↔backend contract: any command/event/type changes are updated in BOTH Rust and TypeScript
- [x] Settings discipline: any settings additions/changes include migrations/normalization and apply immediately at runtime
- [x] Secrets hygiene: no logging of secrets; redact sensitive data in logs
- [x] Tooling gate: plan includes how you’ll keep `pnpm -C app check:ci` green
