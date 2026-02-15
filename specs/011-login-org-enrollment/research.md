# Research: Phase 1 Login and Org Enrollment

## Decision 1: Keep login optional and entitlement additive
- **Decision**: Implement sign-in as an optional capability layer; baseline BYOK/local usage remains fully available while signed out.
- **Rationale**: This preserves OSS-first behavior and avoids regressions for existing community users.
- **Alternatives considered**:
  - Require login for all users (rejected: violates product principle and creates adoption friction).
  - Hide account state entirely when signed out (rejected: reduces discoverability and upgrade clarity).

## Decision 2: Backend-owned entitlement state with cached grace evaluation
- **Decision**: Evaluate entitlement freshness and grace-window transitions in Rust backend state, and expose normalized snapshots to UI via typed commands.
- **Rationale**: Grace logic is security-sensitive and should be centralized in one authoritative runtime.
- **Alternatives considered**:
  - UI-only grace logic (rejected: duplicated logic and higher drift risk across windows).
  - Server-only grace checks without local cache (rejected: poor offline resilience).

## Decision 3: Store only non-secret entitlement cache in settings store
- **Decision**: Persist non-secret entitlement snapshots (tier/org context/cache timestamps) in settings storage; keep auth/session tokens in OS secure storage only.
- **Rationale**: Balances offline resilience with secret hygiene requirements.
- **Alternatives considered**:
  - Store tokens in settings JSON (rejected: security risk).
  - Store no entitlement cache at all (rejected: breaks grace-window behavior).

## Decision 4: Explicit entitlement lifecycle states for UX and diagnostics
- **Decision**: Use clear state transitions (`signed_out`, `active`, `grace`, `expired`) and surface them in account UI and diagnostics-safe events.
- **Rationale**: Reduces support ambiguity and aligns with success criteria for troubleshooting speed.
- **Alternatives considered**:
  - Single boolean active/inactive status (rejected: insufficient clarity for outage/grace scenarios).
  - Hidden lifecycle with generic errors (rejected: poor user/support experience).

## Decision 5: Deterministic test strategy centered on state-machine transitions
- **Decision**: Add deterministic unit tests for cache/expiry transitions in Rust and command-wrapper/UI-state tests in TypeScript with mocked time/input.
- **Rationale**: Meets constitution deterministic test gate and minimizes flaky integration behavior.
- **Alternatives considered**:
  - Live integration tests against external auth services (rejected: non-deterministic and secret-dependent).
  - Manual-only validation (rejected: weak CI confidence).

## References

- `specs/011-login-org-enrollment/spec.md`
- `.specify/memory/constitution.md`
- `plans/KOLBOO_ENTERPRISE_AND_SUBSCRIPTION_READINESS.md` (Phase 1 section)
