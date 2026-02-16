# Quickstart — Phase 3 Managed Inference Proxy

## Prerequisites

- Desktop app workspace: `c:\Users\dovie\repos\kolboo`
- Cloud workspace (for private artifacts): `c:\Users\dovie\repos\kolboo-private`
- Signed-in test identities:
  - Personal active entitlement user
  - Enterprise admin + enterprise member
  - Community (BYOK-only) user

## 1) Implement desktop routing + UX surfaces

1. Add/extend effective inference mode state in desktop settings/policy state normalization.
2. Route managed-eligible requests through managed gateway wrappers.
3. Preserve BYOK path for community and non-managed users.
4. Add usage proximity + quota hard-stop UX messaging using deterministic failure codes.

## 2) Implement gateway contracts and enforcement

1. Add managed gateway endpoints for STT + LLM completion.
2. Require auth token + idempotency key.
3. Enforce order: auth -> entitlement/mode -> quota/rate checks -> downstream call -> idempotent metering.
4. Return deterministic error categories:
   - `unauthorized`
   - `ineligible`
   - `over_quota`
   - `temporarily_unavailable`

## 3) Implement enterprise mode admin flow

1. Add org-level inference mode read/update endpoint.
2. Ensure mode changes propagate to members on sync (<=5 minutes target).
3. Keep org BYOK key-rotation continuity paths working.

## 4) Add privacy-safe telemetry

1. Emit metadata-only request traces (tenant scope, route, status class, latency, failure class).
2. Do not persist transcript/prompt/OCR/audio/credentials in telemetry by default.
3. Ensure observability sink failures do not block request execution.

## 5) Validate behavior

Run formatting/checks first, then smallest validating tests:

1. `pnpm -C app lint`
2. `pnpm -C app test` (TS/UI changes)
3. `pnpm -C app cargo:test` (Rust changes)
4. `pnpm -C app test:all` (cross-surface changes)
5. Final gate once before handoff: `pnpm -C app check:ci`

## 6) Scenario verification checklist

- Personal user can complete first managed request without local API key.
- Over-quota managed request is denied with deterministic actionable message.
- Enterprise org mode toggle (`managed` <-> `byok`) reflects in member behavior after sync.
- Simulated temporary managed outage gives clear fallback/recovery guidance.
- Metering ledger remains exactly-once under retry with same idempotency key.
- Sample telemetry shows only metadata (no content payloads).

## Validation outcomes

- ✅ Managed temporary outage messaging now includes explicit BYOK recovery guidance.
- ✅ Backend managed routing now supports fallback provider resolution when managed gateway credentials are unavailable.
- ✅ Gateway degraded responses normalize to `temporarily_unavailable` with deterministic envelope.
- ✅ Observability emits remain fail-open and non-blocking on sink failure.
- ✅ Added targeted tests:
  - `app/src/lib/queries.managed-fallback.test.ts`
  - `app/src-tauri/src/pipeline/tests/managed_outage_tests.rs`
  - `apps/api-edge/tests/managed-degraded-normalization.test.ts`
  - `packages/observability/tests/managed-redaction.test.ts` (fail-open coverage)