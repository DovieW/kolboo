# Phase 0 Research — Managed Inference Proxy

## Decision 1: Keep desktop enforcement policy-driven, with cloud as source-of-truth for managed eligibility

- **Decision**: Desktop routing will derive effective inference mode from synced entitlement/policy state, while cloud enforces final authorization + quota checks for managed requests.
- **Rationale**: This avoids local drift, keeps enterprise mode changes centrally controlled (FR-003/FR-004), and still preserves responsive desktop UX.
- **Alternatives considered**:
  - Local-only mode flags without policy sync (rejected: inconsistent enterprise behavior).
  - Cloud-only opaque routing with no local effective-mode visibility (rejected: poor UX/debuggability).

## Decision 2: Use deterministic error taxonomy for all managed denials

- **Decision**: Standardize managed gateway failures to `unauthorized`, `ineligible`, `over_quota`, `temporarily_unavailable` with stable machine codes and user-safe messages.
- **Rationale**: Required by FR-007 and improves predictable UX/support workflows.
- **Alternatives considered**:
  - Provider/raw downstream errors surfaced directly (rejected: non-deterministic and potentially sensitive).
  - Generic single failure bucket (rejected: not actionable).

## Decision 3: Enforce quotas and abuse controls before downstream provider calls

- **Decision**: Managed gateway order: auth -> entitlement/mode -> preflight quota/rate controls -> downstream call -> idempotent usage commit.
- **Rationale**: Satisfies FR-006/FR-011/FR-015 and limits spend risk.
- **Alternatives considered**:
  - Post-call quota checks (rejected: cost already incurred).
  - Best-effort only (rejected: insufficient protection).

## Decision 4: Idempotent metering with request keys

- **Decision**: Require request-level idempotency key for managed execution paths; metering ledger commits exactly once per accepted request key.
- **Rationale**: Supports SC-004 and protects against retry double-counting.
- **Alternatives considered**:
  - Timestamp-window dedupe only (rejected: collision/false-positive risk).
  - No dedupe (rejected: violates metering integrity requirements).

## Decision 5: Metadata-only observability by contract

- **Decision**: Telemetry/log contracts will include tenant scope, route, status class, timing, and quota outcome, but exclude transcript/prompt/OCR/audio/credential payloads by default.
- **Rationale**: Required by FR-013/FR-014/SC-006; supports supportability without content retention.
- **Alternatives considered**:
  - Full payload logging with redaction best-effort (rejected: too risky).
  - Minimal/no telemetry (rejected: fails FR-019/SC-007).

## Decision 6: Graceful degradation when managed path is impaired

- **Decision**: On managed-path temporary failure, desktop shows status + fallback guidance and can use configured non-managed path when policy allows.
- **Rationale**: Required by FR-016 and SC-005.
- **Alternatives considered**:
  - Hard fail without fallback UX (rejected: poor reliability/trust).
  - Silent background retry only (rejected: opaque user experience).

## Decision 7: Cloudflare edge rate limiting keyed by tenant/subject, not only IP

- **Decision**: Implement edge rate limiting keyed by authenticated subject/org identifiers and route class.
- **Rationale**: Cloudflare guidance supports custom keys and recommends user identity keys for fair multi-tenant limits.
- **Alternatives considered**:
  - IP-only rate limiting (rejected: weak tenant isolation and shared-network false positives).

## Resolved Clarifications

All initial technical context items are resolved for planning; no remaining `NEEDS CLARIFICATION` entries.
