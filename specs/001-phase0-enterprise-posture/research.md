# Research: Phase 0 Enterprise Posture

## Decision 1: Policy enforcement occurs in settings normalization before runtime sync
- **Decision**: Enforce policy constraints in the existing frontend settings normalization/update flow, then immediately trigger runtime sync and settings-changed propagation.
- **Rationale**: The repo already treats `settings.json` as canonical and uses `syncPipelineConfig` + `settings-changed` to keep runtime/UI aligned. Reusing this path minimizes drift.
- **Alternatives considered**:
  - Backend-only enforcement (rejected: weaker UX transparency and delayed feedback in settings UI).
  - UI-only lock flags without normalization enforcement (rejected: brittle; can drift from effective runtime state).

## Decision 2: Policy source model is forward-compatible but Phase 0 remains local-first
- **Decision**: Keep `PolicyState.source` as `none | file | cloud` but implement behavior that works without login/cloud as Phase 0 baseline.
- **Rationale**: Spec requires no mandatory login in Phase 0 while preparing for future cloud policy. This avoids schema churn later.
- **Alternatives considered**:
  - Use only `none | file` in Phase 0 (rejected: guaranteed migration churn in Phase 1/2).

## Decision 3: Policy diagnostics export is explicitly redacted and support-oriented
- **Decision**: Export policy metadata/effective enforcement outcomes while excluding all secrets and credentials.
- **Rationale**: Support needs actionable diagnostics; constitution requires secret hygiene. Redacted export satisfies both.
- **Alternatives considered**:
  - Raw settings dump (rejected: high risk of leaking keys/tokens).
  - No export (rejected: increases support burden and manual triage time).

## Decision 4: Testing strategy is deterministic and contract-focused
- **Decision**: Add unit tests for normalization/enforcement and diagnostics redaction, plus integration-style tests for policy-change -> sync/event behavior.
- **Rationale**: Constitution requires deterministic tests and UI↔backend contract consistency.
- **Alternatives considered**:
  - Manual-only validation (rejected: insufficient CI signal).
  - End-to-end tests with external services (rejected: violates deterministic/no-network guidance).

## Best-practice Notes

- Keep policy as "boring JSON" with stable schema and explicit null semantics.
- Always emit settings-changed when effective settings are altered by policy.
- Preserve user configurability for unconstrained fields to avoid unnecessary friction.

## References

- `specs/001-phase0-enterprise-posture/spec.md`
- `.specify/memory/constitution.md`
- Existing settings/event flow in `app/src/lib/tauri/settings.ts` and `app/src/lib/tauri/events.ts`
