# Implementation Plan: Identity-Policy Boundary for Desktop Auth

**Branch**: `016-auth-boundary-plan` | **Date**: 2026-02-19 | **Spec**: `./spec.md`
**Input**: Feature specification from `./spec.md`

**Related Docs**:
- Research: `./research.md`
- Data model: `./data-model.md`
- Quickstart: `./quickstart.md`
- Tasks: `./tasks.md`
- Contract: `./contracts/auth-boundary.openapi.yaml`

**Note**: This template is filled in by the `/speckit.plan` command. See `.github/agents/speckit.plan.agent.md` for the execution workflow.

## Summary

Define a secure desktop auth boundary where identity is handled by an external IdP (Supabase Auth first, WorkOS-compatible later) and authorization/policy/metering are enforced at `api-edge`. The implementation path prioritizes browser-based desktop sign-in, secure local session material storage, and bearer-token-based managed calls to edge. Token exchange is designed as a deliberate upgrade path triggered by enterprise needs (multi-IdP abstraction, revocation/kill-switch semantics, claim embedding, rapid IdP-agnostic desktop support).

## Implementation Status Snapshot

- Desktop auth now uses system-browser Authorization Code + PKCE with a loopback callback on `http://127.0.0.1:<random_port>/auth/callback`.
- Session/refresh material is persisted through OS secure storage, while logout and startup refresh keep cached auth/policy state synchronized.
- Managed inference remains bearer-authenticated without regressing BYOK/community behavior.
- Token exchange remains optional: the repo now persists a reviewed trigger set and exposes a placeholder session-exchange surface, but the default decision is still `direct_idp_token`.

## Hard Decisions (Mechanism-Level)

### 1) Desktop sign-in flow

- Adopt **Authorization Code Flow + PKCE** as the required desktop authentication mechanism.
- **Default redirect mechanism**: loopback callback (`http://127.0.0.1:<random_port>/auth/callback`).
- Deep-link callback (`kolboo://auth/callback`) may be added later as a fallback/secondary mechanism, but not as the primary decision for this implementation.
- Password grant is explicitly out of scope as a default login path.

### 2) Token lifecycle and storage rules

- Store refresh/session secret material in **OS secure storage** only.
- Keep access tokens in memory by default; avoid plaintext file persistence.
- Startup must not block full UI paint on refresh/network calls; perform refresh async and gate only privileged managed operations.
- On logout: clear secure storage secrets, clear in-memory auth state, and clear cached policy snapshot.
- On refresh failure: transition to re-auth-required state for managed operations while preserving non-managed/BYOK usability where applicable.

### 3) Edge accepted token type and verification rules

- `api-edge` accepts **IdP access tokens** for managed operations in this phase.
- `api-edge` must validate JWT signature via JWKS and enforce `iss`, `aud`, `exp` (and honor `nbf` when present with bounded clock skew tolerance).
- JWKS key rotation and cache refresh behavior must be implemented to avoid validation outages on key rollover.
- Raw JWTs and authorization headers must never be logged.

### 4) Org/entitlement lookup and cache behavior

- Source of truth for org membership, roles, and entitlements remains product-owned policy data at edge/backend persistence.
- Edge may use a short-lived in-memory authorization snapshot cache (target TTL: 30–120 seconds) for `subject -> policy context`.
- Managed operation authorization fails **closed** when required policy/membership data cannot be resolved.
- Community/BYOK direct flows remain available when edge-managed policy path is unavailable.

### 5) Token exchange gate (actionable)

Adopt token exchange (`/v1/session/exchange`) when **any** of the following are true:

- WorkOS/multi-IdP SSO support is being shipped.
- Revocation/kill-switch semantics are required at org or tenant scope.
- Edge-signed embedded org/tier claims are needed to reduce repeated policy lookups.
- Desktop must become rapidly IdP-agnostic without per-IdP desktop rollout changes.

Current implementation note:

- `settings_version` `8` normalizes and persists `token_exchange_trigger_set` with an explicit decision.
- The desktop command/wrapper surface can report exchange readiness without forcing runtime adoption yet.

## Threat Model and Observability Baseline

### Threat model focus

- Local token theft on compromised machine.
- Token replay attempts.
- MITM/transport interception attempts.
- Secret leakage via logs/diagnostics.

### Observability requirements

- Categorize auth failures by reason (expired, invalid signature, wrong audience/issuer, membership missing, policy denied).
- Track edge auth middleware latency and keep auth context evaluation within the `<300ms p95` goal.
- Ensure telemetry/logging paths redact secrets and raw tokens.

## Technical Context

**Language/Version**: TypeScript (strict) + Rust (Tauri) + Edge API contracts (OpenAPI)
**Primary Dependencies**: React/Vite frontend, Tauri backend, secure secret storage module, cloud `api-edge` auth middleware
**Storage**: OS secure storage for session/refresh material; Tauri store (`settings.json`) for non-secret auth/policy state
**Testing**: Vitest (`pnpm -C app test`), Rust tests (`pnpm -C app cargo:test`), final CI gate (`pnpm -C app check:ci`)
**Target Platform**: Windows desktop primary; macOS/Linux supported by architecture
**Project Type**: Desktop app (Tauri) + cloud edge service contract
**Performance Goals**: Auth context evaluation <300ms p95 at edge; no measurable startup regression from auth state hydration
**Constraints**: No password-grant default flow; no secrets in logs; deterministic tests with no real network in automated suites
**Scale/Scope**: Single product auth boundary; supports Community/Personal/Enterprise modes and future multi-IdP growth

## Contract Baseline for Implementation

- `GET /v1/auth/context` (or equivalent `/me` snapshot): returns user/org/entitlement context for managed mode.
- `POST /v1/managed/inference`: requires bearer auth and policy allow.
- `POST /v1/session/exchange`: optional endpoint activated by token-exchange gate criteria.

## Constitution Check

_GATE: Must pass before Phase 0 research. Re-check after Phase 1 design._

- [x] Deterministic tests: no real network calls in tests; no real API keys required by default
- [x] UI↔backend contract: any command/event/type changes are updated in BOTH Rust and TypeScript
- [x] Settings discipline: any settings additions/changes include migrations/normalization and apply immediately at runtime
- [x] Secrets hygiene: no logging of secrets; redact sensitive data in logs
- [x] Tooling gate: plan includes how you’ll keep `pnpm -C app check:ci` green

**Gate Status (Pre-Research)**: PASS

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

```text
app/
├── src/                # React/TypeScript UI
├── src-tauri/src/      # Rust/Tauri backend
└── tests/              # (if present) test helpers, fixtures, etc.

docs/
scripts/
```

**Structure Decision**: Keep implementation split across existing Tauri boundaries: desktop auth/session orchestration in `app/src-tauri/src/**`, frontend auth UX/state wiring in `app/src/**`, and edge API behavior documented via `specs/016-auth-boundary-plan/contracts/**`. No new top-level modules are required.

## Post-Design Constitution Check

- [x] Deterministic tests preserved in design (all contract/unit tests mock IdP and edge responses)
- [x] UI↔backend contract explicitly captured (auth state/events and request wrappers are versioned in one contract set)
- [x] Settings and secure-storage responsibilities are separated and documented
- [x] Secrets hygiene preserved (no token logging; explicit redaction requirements in quickstart)
- [x] Validation path includes formatting, targeted tests, then final `pnpm -C app check:ci`

**Gate Status (Post-Design)**: PASS

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
| --------- | ---------- | ------------------------------------ |
| None      | N/A        | N/A                                  |
