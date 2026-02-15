# Quickstart: Phase 2 Cloud Policy Packs

## Goal

Implement and validate cloud policy pack consumption/enforcement for enrolled enterprise users while keeping offline reliability and baseline OSS behavior intact.

## Implementation Steps

1. Add backend policy module and commands:
   - Add `policy.rs` with validation, cache loading/saving, and effective settings merge.
   - Add `commands/policy.rs` for `policy_sync`, `policy_get_state`, and `policy_export_diagnostics`.
   - Register commands and events (`policy-state-changed`, existing `settings-changed`) in `lib.rs`.
2. Extend shared types/contracts:
   - Add TS/Rust mirrored policy state and diagnostics payload types.
   - Add TS wrappers under `app/src/lib/tauri/**` for new commands.
3. Enforce settings consistently:
   - Apply policy constraints in shared settings normalization/runtime sync path.
   - Ensure policy-enforced settings are immutable from UI actions.
4. Implement UI visibility:
   - Show enforcement indicators and reason labels for locked controls.
   - Add policy diagnostics panel showing source/version/timestamps/expiry.
5. Add diagnostics export:
   - Export JSON artifact with policy metadata + enforcement summary.
   - Redact secrets and user content.

## Verification Steps

1. Formatting (first):
   - `pnpm -C app lint`
   - `pnpm -C app cargo:fmt`
2. Targeted tests during iteration:
   - `pnpm -C app test:all`
3. Final gate before handoff:
   - `pnpm -C app check:ci`

## Manual Acceptance Checklist

- Sync applies valid policy and updates UI within expected latency target.
- Enforced settings cannot be changed by user interactions.
- Unsupported/invalid policy updates are rejected with last valid policy preserved.
- During simulated outage, cached policy remains active until expiry; then degraded state appears.
- Diagnostics export contains policy metadata/enforcement outcomes with no secrets.

## Validation Results (2026-02-14)

- ✅ `pnpm -C app lint`
- ✅ `pnpm -C app cargo:fmt`
- ✅ `pnpm -C app test` (31 files: 30 passed, 1 skipped)
- ✅ `pnpm -C app cargo:test` (470 passed, 11 ignored)
- ✅ `pnpm -C app check:ci`

Notes:
- `cargo:clippy:ci` reports a pre-existing warning in `src/cli/pipeline.rs` (`clone_on_copy`); check remains successful.
