# Quickstart: Phase 0 Enterprise Posture

## Goal
Implement policy enforcement posture without requiring login/cloud inference:
- enforce constrained settings
- provide policy transparency UI
- support redacted diagnostics export

## Steps

1. **Add policy state + normalization hooks**
   - Extend settings normalization layer to compute effective settings from user settings + policy constraints.
   - Track `PolicyState` (`none|file|cloud`, last_updated, expiry, validity).

2. **Enforce constrained fields in settings UX**
   - Mark policy-controlled settings as read-only/locked with reason text.
   - Prevent mutation attempts for constrained fields.

3. **Apply changes immediately at runtime**
   - After policy-driven effective settings changes, persist and trigger runtime sync.
   - Emit `settings-changed` so dependent windows/state refresh.

4. **Add dedicated policy view**
   - Show policy source, status, update time, and enforcement summary.
   - Include clear user-facing explanations for controlled fields.

5. **Implement diagnostics export**
   - Export policy metadata and enforcement outcomes.
   - Ensure strict redaction of secrets/credentials.

6. **Tests (deterministic)**
   - TS unit tests: normalization + enforcement locks + unconstrained edits.
   - Rust/TS tests: policy-changed -> sync/event behavior where applicable.
   - Diagnostics tests: verify secret redaction.

## Validation commands

1. Format/lint first: `pnpm -C app lint`
2. Run relevant tests: `pnpm -C app test` and/or `pnpm -C app cargo:test`
3. Final gate: `pnpm -C app check:ci`

## Out-of-scope for Phase 0

- Managed inference proxy behavior
- Billing/subscription flows
- Mandatory login/account enforcement
- Enterprise SSO implementation

## Validation results (2026-02-15)

- ✅ `pnpm -C app lint`
- ✅ `pnpm -C app test` (31 files: 30 passed, 1 skipped)
- ✅ `pnpm -C app check:ci`

Notes:
- `cargo:clippy:ci` still reports the existing `clone_on_copy` warning in `app/src-tauri/src/cli/pipeline.rs`; CI gate remains successful.
