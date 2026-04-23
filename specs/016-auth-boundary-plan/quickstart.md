# Quickstart: Identity-Policy Boundary for Desktop Auth

**Feature Branch**: `016-auth-boundary-plan`
**Related Docs**:
- Spec: `./spec.md`
- Plan: `./plan.md`
- Research: `./research.md`
- Data model: `./data-model.md`
- Tasks: `./tasks.md`
- Contract: `./contracts/auth-boundary.openapi.yaml`

## Goal

Implement and validate the desktop auth boundary where:

1. Desktop uses browser-based PKCE sign-in.
2. Session/refresh material is stored in OS secure storage.
3. Managed operations call `api-edge` with bearer authentication.
4. Edge validates identity token, evaluates org/entitlement policy, meters usage, and routes managed inference.
5. Token exchange remains optional until trigger criteria are met.

## Current Desktop Snapshot

- Desktop sign-in now uses the system browser plus Supabase Authorization Code + PKCE.
- The loopback callback target is `http://127.0.0.1:<random_port>/auth/callback`.
- Session and refresh secrets stay in OS secure storage; non-secret auth state remains in settings.
- Managed inference continues to attach desktop bearer auth, while BYOK/community paths stay login-optional.
- Token exchange readiness is exposed as a placeholder command/contract, but the current default decision remains `direct_idp_token` until a trigger is explicitly enabled.

## Implementation Sequence (Minimal, Non-Overbuilt)

1. **Desktop auth UX/state**
   - Add/confirm browser-based sign-in initiation and callback handling.
   - Persist only non-secret auth UI state in settings.

2. **Secure session handling**
   - Persist refresh/session material through secure storage APIs.
   - Ensure logout clears secure material and resets auth state.

3. **Edge-authenticated managed path**
   - Attach bearer token for managed edge calls.
   - Keep Community/BYOK paths independent from managed auth requirement.

4. **Edge authorization/metering behavior**
   - Validate JWT (`iss`, `aud`, `exp`) and key material.
   - Resolve org membership/tier/entitlements.
   - Return explicit deny reason codes for auth/policy failures.
   - Record usage for successful managed operations.

5. **Token exchange gate**
   - Add architecture flag or planning marker for `/v1/session/exchange` readiness.
   - Do not force adoption unless trigger criteria are met.

## Validation Strategy

### Deterministic tests

- Unit tests must mock IdP tokens/JWKS and edge responses.
- No real network calls in automated suites.
- No real API keys required for default test runs.

### Suggested local checks

- TypeScript/UI-focused updates:
  - `pnpm -C app test`
- Rust/backend-focused updates:
  - set `RUSTC_WRAPPER=sccache` when available
  - set `CARGO_BUILD_JOBS` to conservative value
  - `pnpm -C app cargo:test`
- Cross-cutting auth boundary changes:
  - `pnpm -C app test:all`
- Final gate before merge:
  - `pnpm -C app check:ci`

## Operational Guardrails

- Never log tokens or raw authorization headers.
- Redact sensitive fields in diagnostics and telemetry.
- Keep UI↔backend command/event/type contracts aligned for any changed auth flows.
- If token exchange is activated, document migration path and session invalidation semantics.

## Trigger Checklist for Token Exchange Adoption

Adopt token exchange when one or more are true:

- WorkOS or multi-IdP support required.
- Revocation / kill-switch semantics required.
- Need edge-signed org/tier claims to reduce repeated lookup overhead.
- Need fast desktop IdP-agnostic behavior.

## Operational Trigger Review Checklist

Use this review whenever auth architecture changes are proposed or before enabling `/v1/session/exchange`.

1. Confirm whether each trigger is currently required:
    - `multi_idp_required`
    - `kill_switch_required`
    - `embedded_claims_required`
    - `desktop_idp_agnostic_required`
2. Record the review date and owner in `research.md`.
3. Verify the persisted `token_exchange_trigger_set` matches the intended booleans and review timestamp.
4. If any trigger is `true`, set the decision to `adopt_token_exchange` and treat the placeholder exchange path as implementation-ready work.
5. If all triggers are `false`, keep the decision `direct_idp_token` and avoid introducing exchange-only runtime dependencies.
6. Re-run desktop auth validation after any trigger change so sign-in, logout, and managed-call gating still behave the same for current users.

### Current review outcome

- Review date: 2026-04-22
- Decision: `direct_idp_token`
- Active triggers: none
- Follow-up: keep `/v1/session/exchange` as a readiness placeholder until an enterprise trigger flips on.

## Latest Validation Outcomes

- 2026-04-22 — `pnpm -C app test`
   - Result: ✅ passed
   - Summary: `40 passed | 1 skipped` test files; `363 passed | 57 skipped` tests
- 2026-04-22 — `pnpm -C app cargo:test`
   - Result: ✅ passed
   - Summary: `501 passed | 0 failed | 11 ignored`
- 2026-04-22 — `pnpm -C app lint`
   - Result: ✅ passed after formatting touched files (`3 files` updated)
- 2026-04-22 — `pnpm -C app cargo:fmt`
   - Result: ✅ passed
- 2026-04-22/23 — `pnpm -C app check:ci`
   - Result: ✅ passed
   - Summary: lint, typecheck, knip, schema/event/type generation checks, Vitest, Cargo fmt check, and Cargo test CI all passed; Clippy reported non-fatal warnings in pre-existing untouched Rust paths.
