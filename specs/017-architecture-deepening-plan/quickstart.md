# Quickstart: Architecture Deepening Plan

This quickstart explains how to use the Spec Kit artifacts for `017-architecture-deepening-plan` and how to validate implementation slices when tasks are generated.

## 0. Active feature verification

Implementation setup verified on 2026-05-03 that `.specify/feature.json` points to `specs/017-architecture-deepening-plan`. Because the current Git branch is `master`, local Spec Kit prerequisite checks use `SPECIFY_FEATURE=017-architecture-deepening-plan` instead of switching branches.

## 1. Read the planning artifacts

Start here:

1. `specs/017-architecture-deepening-plan/spec.md`
2. `specs/017-architecture-deepening-plan/plan.md`
3. `specs/017-architecture-deepening-plan/research.md`
4. `specs/017-architecture-deepening-plan/data-model.md`
5. `specs/017-architecture-deepening-plan/contracts/`

The plan is staged. Do not implement all opportunities in one unreviewable burst.

## 2. Implementation order

Recommended slice order:

1. OCR Session state/interface deepening
2. Settings defaults and Settings View
3. Settings Runtime Sync Policy
4. Transcription Flow Routing Decision
5. Profile matching and effective profile behavior split
6. Local Provider Lifecycle
7. Provider-Family Seam pre-flight decisions and selected real seams

Each slice must be independently testable and safe to stop after completion.

## 3. Coverage gate

For every changed or newly introduced in-scope module:

- 100% statement coverage
- 100% branch coverage
- 100% function coverage
- deterministic tests for documented edge cases
- regression tests for every bug found during implementation

TypeScript coverage currently uses:

```text
pnpm -C app coverage
```

Rust coverage must be added before Rust slices claim completion. Prefer a deterministic Rust coverage path such as `cargo llvm-cov` wired through `pnpm -C app cargo:coverage`. Until that exists, Rust slices may pass tests but cannot claim the spec's 100% coverage gate. The expected helper behavior is documented in `specs/017-architecture-deepening-plan/validation/rust-coverage-interface.md`.

## 4. Determinism rules

Default validation must not require:

- real network calls
- real API keys
- paid accounts or quotas
- real audio devices
- screenshots or OCR providers
- timing sleeps

Use fake providers, fake tasks, fixture settings snapshots, controlled cancellation, and local deterministic fixtures.

## 5. Smallest validation command set

Use the smallest command set that proves the slice, then escalate only as needed.

### Docs/spec-only updates

No app validation required unless the docs reference generated behavior.

### TypeScript/UI-only slice under `app/src/**`

```text
pnpm -C app lint
pnpm -C app test
pnpm -C app coverage
```

### Rust-only slice under `app/src-tauri/**`

Before commands that invoke Cargo, configure local Rust cache/jobs in the current PowerShell session:

```text
if (Get-Command sccache -ErrorAction SilentlyContinue) { $env:RUSTC_WRAPPER = "sccache" } else { Remove-Item Env:RUSTC_WRAPPER -ErrorAction SilentlyContinue }
$cores = [Environment]::ProcessorCount
$env:CARGO_BUILD_JOBS = [Math]::Min(8, [Math]::Max(1, [Math]::Floor($cores / 2)))
pnpm -C app cargo:fmt
pnpm -C app cargo:test
```

Add/run `pnpm -C app cargo:coverage` for the slice before completion once the US8 coverage helper is implemented.

### Cross-cutting TypeScript + Rust slice

```text
if (Get-Command sccache -ErrorAction SilentlyContinue) { $env:RUSTC_WRAPPER = "sccache" } else { Remove-Item Env:RUSTC_WRAPPER -ErrorAction SilentlyContinue }
$cores = [Environment]::ProcessorCount
$env:CARGO_BUILD_JOBS = [Math]::Min(8, [Math]::Max(1, [Math]::Floor($cores / 2)))
pnpm -C app test:all
pnpm -C app coverage
```

Run `pnpm -C app check:ci` once at the end of the full initiative or before merge, not after every small iteration.

## 6. Slice done checklist

A slice is complete only when:

- module interface contract is satisfied
- edge-case matrix has deterministic tests
- 100% in-scope coverage evidence exists
- no new VS Code Problems remain in touched files
- formatting ran before tests/checks
- affected contracts/schemas/generated files are synchronized
- documentation/refactor notes are updated where behavior or decisions changed
- safe-stop/rollback notes are recorded

## 7. Next Spec Kit step

Generate tasks with:

```text
/speckit.tasks
```

Before task generation, consider using `/speckit.analyze` for consistency review because this plan intentionally spans many modules and quality gates.
