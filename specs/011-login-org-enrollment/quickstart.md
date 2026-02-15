# Quickstart: Phase 1 Login and Org Enrollment

## Scope

Implement optional login, account/org visibility, and offline entitlement grace behavior for Kolboo desktop.

Related docs:
- Plan: `specs/011-login-org-enrollment/plan.md`
- Tasks: `specs/011-login-org-enrollment/tasks.md`

## Prerequisites

- Node/pnpm environment working for `app/`
- Rust toolchain for Tauri backend
- No real auth keys required for automated tests (use mocks/fakes)

## Implementation Steps

1. Add backend entitlement/account domain module (`app/src-tauri/src/licensing.rs`) with:
   - `LicenseState` model and status transitions (`signed_out`, `active`, `grace`, `expired`)
   - Grace-window evaluator using cached timestamps
   - Redaction-safe diagnostics transition events
2. Add Tauri commands (`app/src-tauri/src/commands/licensing.rs`) for:
   - state read
   - login start/complete callback handling
   - logout
   - entitlement refresh
   - management URL
3. Register commands/module in `app/src-tauri/src/lib.rs` and command mod index.
4. Add TS wrappers/types (`app/src/lib/tauri/commands.ts`, `types.ts`, new `license.ts`).
5. Add query hooks for account state in `app/src/lib/queries.ts`.
6. Add account settings UI surface and user-facing errors in `app/src/components/settings/` and `app/src/App.tsx`.
7. Ensure settings/runtime sync for any new persisted entitlement cache fields.
8. Add deterministic tests:
   - Rust unit tests for state transitions and grace boundaries
   - TS wrapper/query/UI tests for signed-in/out + org visibility + failure messaging
9. Add Phase 1 Sentry telemetry:
   - Initialize Sentry on desktop surfaces (`main`, `overlay`, `overlay_hover`, `quick_ask`) with DSN gating.
   - Add shared redaction helpers so account/licensing telemetry strips tokens/secrets and user content.
   - Wrap account/licensing command wrappers with Sentry error capture and redacted context.

## Validation Flow

Run formatting first, then checks/tests.

1. `pnpm -C app lint`
2. `pnpm -C app test`
3. `pnpm -C app cargo:test`
4. `pnpm -C app check:ci`

### Sentry-specific Validation

1. Confirm local `.env` has placeholder Sentry variables and empty DSN by default:
   - `VITE_SENTRY_DSN=`
   - `VITE_SENTRY_ENV=development`
   - `VITE_APP_VERSION=`
2. Run deterministic tests that verify telemetry redaction paths:
   - `app/src/lib/tauri/license.test.ts`
   - `app/src-tauri/src/licensing.rs` (`telemetry_context_redacts_sensitive_fields`)
3. Confirm app behavior with DSN unset (no startup/runtime failures).

## Latest Validation Snapshot

- `pnpm -C app lint` ✅
- `pnpm -C app typecheck` ✅
- `pnpm -C app test` ✅ (`305 passed`, `57 skipped`)
- `pnpm -C app cargo:test` ✅ (`463 passed`, `11 ignored`)
- `pnpm -C app check:ci` ✅
   - Includes lint, typecheck, knip, schema/event/type parity, Vitest, clippy, rustfmt check, and Rust CI tests (`463 passed`, `11 ignored`).

## Manual Verification

1. Launch app and verify signed-out mode keeps baseline features functional.
2. Simulate successful sign-in and verify account identity + tier render.
3. Simulate org membership and verify org context appears.
4. Simulate entitlement refresh failures and verify grace indicator appears.
5. Advance mocked/stubbed time beyond grace and verify predictable downgrade.
6. Sign out and verify account-only state clears while baseline usage remains.
