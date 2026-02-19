# Implementation Plan: Finish Remaining Phase 5/5A/5B Readiness

**Branch**: `015-finish-phase5-readiness` | **Date**: 2026-02-18 | **Spec**: `c:\Users\dovie\repos\kolboo\specs\015-finish-phase5-readiness\spec.md`
**Input**: Feature specification from `/specs/015-finish-phase5-readiness/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.github/agents/speckit.plan.agent.md` for the execution workflow.

## Summary

Complete remaining enterprise-admin readiness by delivering full org admin controls (members, policy, usage/audit, shared profiles, billing access), deterministic non-production validation workflows (local/preview/staging), and Phase 5B platform operations via `kolops`. Implementation remains split by repository ownership: enterprise/admin cloud workflows in `kolboo-private`, with targeted desktop integration touchpoints in `kolboo` for enterprise surfaces and non-production persona/test-access visibility.

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: TypeScript (strict), Rust (Tauri), Node.js scripts (workspace standard)
**Primary Dependencies**: React/Vite, TanStack Query, Supabase auth/data access, Cloudflare Workers routes, Tauri command wrappers, `kolops` CLI app runtime
**Storage**: Supabase Postgres for org/admin data; Tauri store (`settings.json`) and secure storage for desktop integration touchpoints
**Testing**: Vitest (dashboard/api-edge/contracts/scripts), Rust unit/integration tests where desktop backend is touched, deterministic smoke/script checks
**Target Platform**: Cloud services (`kolboo-private`) + Desktop app (`kolboo`) with Windows-first local validation
**Project Type**: Cross-repo implementation (open-core split: `kolboo` desktop + `kolboo-private` services)
**Performance Goals**: Admin API p95 within existing Phase 5 benchmark target (<= 300ms for benchmarked endpoints); preview seed/reset cycle < 15 minutes end-to-end
**Constraints**: Deterministic tests only, non-production-only test access hard-stop, no secret leakage in logs, no permanent production backdoor
**Scale/Scope**: 5 admin workflow surfaces + 3 stable staging personas + 6 `kolops` command groups + release evidence gating

## Constitution Check

_GATE: Must pass before Phase 0 research. Re-check after Phase 1 design._

- [x] Deterministic tests: no real network calls in tests; no real API keys required by default
- [x] UI↔backend contract: any command/event/type changes are updated in BOTH Rust and TypeScript
- [x] Settings discipline: any settings additions/changes include migrations/normalization and apply immediately at runtime
- [x] Secrets hygiene: no logging of secrets; redact sensitive data in logs
- [x] Tooling gate: plan includes how you’ll keep `pnpm -C app check:ci` green

Gate status: PASS (no constitutional violations identified in planning scope).

## Project Structure

### Documentation (this feature)

```text
specs/015-finish-phase5-readiness/
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
kolboo/
├── app/
│   ├── src/                      # desktop React UI integration touchpoints
│   └── src-tauri/src/            # desktop backend command/event integration touchpoints
└── specs/015-finish-phase5-readiness/

kolboo-private/
├── apps/admin-dashboard/         # org admin UI features (members/policy/usage/shared-profile/billing)
├── apps/api-edge/                # org admin APIs + non-prod test access + release evidence routes
├── apps/kolops-cli/              # Phase 5B platform admin CLI
├── docs/                         # runbooks (5A/0B/operator)
├── scripts/phase0b/              # seed/reset/smoke operational scripts
└── tests/phase5/                 # benchmark and smoke script tests
```

**Structure Decision**: Implement feature behavior where ownership already exists: admin cloud features and operator flows in `kolboo-private`; desktop-specific enterprise affordances and settings/event visibility in `kolboo`. Keep contracts in spec artifacts and implementation repos in lockstep.

## Phase 0 Research Plan

Generate `research.md` to lock decisions for:

1. Remaining Phase 5/5A/5B scope boundaries and sequencing
2. Desktop vs cloud ownership boundaries for each requirement
3. Deterministic non-production validation model (seed/reset/personas)
4. `kolops` command contract and audit expectations
5. Security guardrails (non-prod hard stop + secret redaction)

## Phase 1 Design Plan

Generate:

- `data-model.md`: entities, relationships, validations, state transitions
- `contracts/`: admin API and `kolops` command contracts for remaining scope
- `quickstart.md`: end-to-end validation flow (local → preview → staging → prod)

Then run agent context refresh:

- `.specify/scripts/powershell/update-agent-context.ps1 -AgentType copilot`

## Post-Design Constitution Check

- [x] Deterministic tests preserved in design artifacts
- [x] UI↔backend contract sync enforced by explicit contracts and desktop touchpoint requirement
- [x] Settings discipline accounted for in desktop integration requirements
- [x] Secrets hygiene enforced in runbook/ops constraints and release evidence handling
- [x] CI/tooling gate retained as final implementation validation requirement

Post-design gate status: PASS.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No constitutional violations requiring exceptions.
