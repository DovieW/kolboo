# Research: Phase 2 Cloud Policy Packs

## Decision 1: Policy integrity validation lives in Rust and blocks application on any verification failure

- **Decision**: Perform integrity validation in `app/src-tauri/src/policy.rs` before any policy is normalized or persisted. Validation includes payload schema checks, issuer/audience checks, and signature verification using a pinned org policy key set.
- **Rationale**: Trust boundaries should be backend-owned; Rust side is already the source of truth for runtime pipeline/state transitions and avoids duplicate trust logic in multiple UI windows.
- **Alternatives considered**:
  - Validate in TypeScript only: rejected due to weaker trust boundary and duplication risk.
  - Best-effort validation with warning-only fallback: rejected because FR-003/FR-004 require rejecting unverifiable updates.

## Decision 2: Use stale-while-degraded cache model with explicit expiry

- **Decision**: Persist the last valid `PolicyPack` and `PolicyState` in settings store; continue enforcing cached policy while `now <= expires_at`; transition to `degraded_expired` when expired and sync still failing.
- **Rationale**: Satisfies FR-005 and FR-013 while preserving deterministic behavior and minimizing outage impact.
- **Alternatives considered**:
  - Immediate disable on first fetch failure: rejected (poor resilience, violates outage story intent).
  - Unlimited cache usage: rejected (no bounded trust window).

## Decision 3: Enforcement happens in settings normalization layer, not only in UI controls

- **Decision**: Apply policy overrides and lock semantics in shared settings normalization/runtime-application path, then render lock indicators in UI.
- **Rationale**: Prevents bypass through alternate surfaces (quick ask, overlays, background updates) and keeps effective behavior consistent across windows.
- **Alternatives considered**:
  - UI-only disabling controls: rejected because backend/runtime could diverge.
  - Backend-only silent overrides without UI indicators: rejected; violates FR-008 transparency.

## Decision 4: Contract shape for policy sync + diagnostics

- **Decision**: Expose explicit commands for `policy_sync`, `policy_get_state`, and `policy_export_diagnostics`; emit `settings-changed` and `policy-state-changed` events when policy state/effective settings change.
- **Rationale**: Matches existing invoke/event architecture and keeps UI and backend contract explicit and testable.
- **Alternatives considered**:
  - Poll-only model: rejected due to slower propagation and poorer UX.
  - Single monolithic command returning all state: rejected as less composable and harder to test incrementally.

## Decision 5: Testing strategy remains deterministic and layered

- **Decision**: Use pure unit tests for policy validation/merge rules (Rust + TS), command wrapper tests for invoke contracts, and integration-ish flow tests with mocked fetch/clock sources.
- **Rationale**: Aligns with constitution deterministic-testing principle and keeps CI reliable.
- **Alternatives considered**:
  - Real policy service integration tests in default suite: rejected due to network/key dependency.
  - Time-based sleep assertions for expiry: rejected; use injected clock to avoid flakes.
