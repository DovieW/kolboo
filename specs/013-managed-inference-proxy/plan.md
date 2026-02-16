# Implementation Plan: Phase 3 Managed Inference Proxy

**Branch**: `013-managed-inference-proxy` | **Date**: 2026-02-15 | **Spec**: `c:\Users\dovie\repos\kolboo\specs\013-managed-inference-proxy\spec.md`
**Input**: Feature specification from `c:\Users\dovie\repos\kolboo\specs\013-managed-inference-proxy\spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.github/agents/speckit.plan.agent.md` for the execution workflow.

## Summary

Deliver the full Phase 3 managed inference proxy slice so signed-in Personal users can run inference without local provider keys, Enterprise orgs can enforce managed vs org-BYOK mode, and operators can enforce quotas/abuse controls with metadata-only observability. The implementation spans desktop routing/metering UX in `kolboo` and cloud gateway + quota/mode enforcement contracts in `kolboo-private`, with deterministic error/status behavior and privacy-safe telemetry.

## Technical Context

**Language/Version**: TypeScript (strict) + Rust (Tauri) for desktop; TypeScript runtime contracts for cloud gateway/control-plane scaffolding
**Primary Dependencies**: React/Vite, TanStack Query, `@tauri-apps/api`, Tauri Rust backend pipeline/state-machine, Cloudflare Worker patterns (JWT + rate limit + deterministic status)
**Storage**: Tauri store (`settings.json`) for local settings/cache, Supabase Postgres-backed entitlement/usage counters in cloud, request-log metadata surfaces
**Testing**: Vitest (`pnpm -C app test`) + Rust tests (`pnpm -C app cargo:test`) + cross-surface validation (`pnpm -C app test:all` and final `pnpm -C app check:ci`)
**Target Platform**: Windows desktop primary, macOS/Linux secondary, Cloudflare edge runtime for managed gateway paths
**Project Type**: Multi-repo open-core product (`kolboo` desktop + `kolboo-private` cloud services)
**Performance Goals**: Personal users complete first managed inference in <2 minutes from launch (SC-001); deterministic mode-change reflection in <=5 minutes (SC-003)
**Constraints**: No transcript/audio/prompt/OCR content persisted in cloud telemetry; no observability-path hard dependency for request success; idempotent metering required
**Scale/Scope**: Phase 3 full slice across user routing, enterprise mode enforcement, quota/metering, outage behavior, and operator diagnostics

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] Deterministic tests: all new/updated tests use fakes/stubs and avoid real network/API keys
- [x] UI↔backend contract: any command/event/type surface updates are planned in Rust + TypeScript wrappers/types together
- [x] Settings discipline: managed-mode related settings/policy-derived state changes must persist + trigger runtime sync immediately
- [x] Secrets hygiene: proxy/auth/telemetry paths redact keys/tokens/headers and avoid sensitive payload logging
- [x] Tooling gate: validate with repo command ladder and final `pnpm -C app check:ci`

Post-design re-check: PASS (no constitution violations requiring exception)

## Project Structure

### Documentation (this feature)

```text
specs/013-managed-inference-proxy/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── managed-inference-gateway.openapi.yaml
│   └── inference-mode-admin.openapi.yaml
└── tasks.md             # created later by /speckit.tasks
```

### Source Code (repository root)

```text
app/
├── src/
│   ├── components/settings/
│   ├── lib/queries.ts
│   └── lib/tauri/{commands.ts,types.ts,settings.ts}
├── src-tauri/src/
│   ├── pipeline.rs
│   ├── commands/{config.rs,stats.rs}
│   └── cost/

kolboo-private/
├── apps/api-edge/
├── apps/policy-control-plane/
├── apps/admin-dashboard/
├── packages/contracts/
└── packages/policy-core/

docs/
scripts/
```

**Structure Decision**: Use existing split architecture: desktop runtime/routing and UX in `kolboo/app/**`; cloud proxy/mode/quota contracts and admin surfaces in `kolboo-private/apps/**` with shared contracts under `kolboo-private/packages/contracts/**`.

## Complexity Tracking

No constitutional violations or complexity exceptions required.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| N/A | N/A | N/A |
