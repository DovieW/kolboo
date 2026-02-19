# Phase 0 Research — Finish Remaining Phase 5/5A/5B Readiness

## Decision 1: Keep implementation ownership split by repo boundary

- **Decision**: Implement org admin services, dashboard, test-access tooling, and `kolops` in `kolboo-private`; implement only required enterprise desktop integration touchpoints in `kolboo`.
- **Rationale**: This matches the open-core ownership model and avoids duplicating cloud/admin logic in the desktop repository.
- **Alternatives considered**:
  - Move admin capabilities into desktop app: rejected due to architecture mismatch and increased security risk.
  - Keep all work cloud-only: rejected because spec requires desktop enterprise indicators/touchpoints.

## Decision 2: Treat remaining scope as three deliverable slices (5, 5A, 5B)

- **Decision**: Plan work in explicit slices: (a) admin workflows (5), (b) deterministic test path and safety controls (5A), (c) operator CLI operations (5B).
- **Rationale**: Independent validation per slice reduces risk and enables staged delivery.
- **Alternatives considered**:
  - Single combined rollout: rejected for high integration risk and slower feedback.

## Decision 3: Use deterministic seeded personas for local/preview/staging validation

- **Decision**: Standardize 3 personas (BYOK org, managed org, mixed-policy org) with context-scoped seed/reset behavior and scripted smoke checks.
- **Rationale**: Reproducible, no-customer dependency validation aligned with constitution deterministic-test rule.
- **Alternatives considered**:
  - Manual ad hoc test data: rejected as non-repeatable and error-prone.

## Decision 4: Maintain strict non-production hard-stop for test-access features

- **Decision**: Keep fixture/test-access routes hard-blocked in production with deterministic error responses and require audit records for non-production sessions.
- **Rationale**: Prevents accidental production backdoors while preserving rapid QA workflows.
- **Alternatives considered**:
  - Feature-flag-only guard: rejected as insufficient without environment hard-stop.

## Decision 5: Define `kolops` as auditable operator interface, not deployment orchestrator

- **Decision**: Scope `kolops` to org/policy/key/entitlement/usage/audit operations only; deployment orchestration remains CI/scripts.
- **Rationale**: Clear responsibility boundary and smaller attack surface for privileged commands.
- **Alternatives considered**:
  - Add deploy operations to `kolops`: rejected due to overlap with existing CI gates and approval controls.

## Decision 6: Keep contract-first alignment across API, CLI, and desktop touchpoints

- **Decision**: Publish contracts in this spec for remaining admin APIs and `kolops` command interface; ensure desktop touchpoint requirements are explicitly tracked.
- **Rationale**: Minimizes drift between spec intent and implementation across repos.
- **Alternatives considered**:
  - Documentation-only requirements without contract artifacts: rejected due to drift risk.

## Decision 7: Validate through smallest deterministic command set first, then final CI gate

- **Decision**: During implementation use targeted checks per touched area; run full CI gate once at handoff.
- **Rationale**: Faster iteration while preserving merge confidence and constitutional tooling constraints.
- **Alternatives considered**:
  - Re-running full CI gate each change: rejected for slow feedback loops.
