# Contract: 100% In-Scope Coverage and Validation Gate

**Feature**: 017 Architecture Deepening Plan
**Status**: Draft validation contract for implementation tasks

## Coverage scope

The coverage gate applies to every module that is changed or newly introduced as part of this initiative and every behavior reachable through the module interfaces defined in `module-interface-contracts.md`.

Coverage is not claimed globally for untouched historical code unless a task explicitly expands scope.

## Required metrics

For each in-scope changed or new module:

- 100% statement coverage
- 100% branch coverage
- 100% function coverage
- 100% documented edge-case coverage
- regression coverage for every bug found during the slice

## Evidence requirements

Each implementation slice must produce or update validation evidence that lists:

- modules in scope
- tests covering normal behavior
- tests covering edge/error/cancellation/fallback behavior
- coverage command used
- coverage result
- any manual/ignored tests and why they do not replace default validation
- any unreachable paths and the explicit scope decision for them

## Deterministic validation rules

Default automated validation MUST NOT require:

- real network calls
- real API keys
- paid accounts or quotas
- real audio hardware
- screenshots
- timing sleeps
- user interaction

Default automated validation SHOULD use:

- fake providers
- fake OCR tasks
- deterministic channels or controlled cancellation
- fixture settings snapshots
- local request-log fixtures
- fake runtime sync/event adapters
- local-only provider behavior doubles

## TypeScript validation commands

Use existing commands unless tasks deliberately add more specific scripts:

```text
pnpm -C app lint
pnpm -C app test
pnpm -C app coverage
```

For final cross-cutting validation:

```text
pnpm -C app check:ci
```

## Rust validation commands

Before Cargo commands on local machines, set `RUSTC_WRAPPER=sccache` when available and set conservative `CARGO_BUILD_JOBS`.

Use existing commands:

```text
pnpm -C app cargo:fmt
pnpm -C app cargo:test
```

Rust coverage command is a required implementation task because the repo currently exposes Rust tests but not a package-scripted Rust coverage command. The selected command must be deterministic and documented before any Rust slice claims completion.

## Branch and edge-case matrix

Each slice must maintain a matrix mapping spec edge cases to tests. At minimum:

| Area | Required edge families |
|------|------------------------|
| OCR Session | stale success, stale failure, await timeout restore, explicit cancel, repeated cancel/end, sanitized failure |
| Settings View | absent keys, explicit null, malformed values, legacy values, policy-enforced values, profile inheritance |
| Runtime Sync Policy | pipeline-only, event-only, both, neither, batch dedupe, API-key/policy/license changes |
| Routing Decision | selected preset, default target, no decision, ambiguity, failure, unknown id, cancellation |
| Profile Resolution | full path, basename, case/prefix/suffix normalization, no foreground, disabled/invalid profile, OCR flow precedence |
| Local Provider Lifecycle | manual unloaded, loaded reuse, config change, managed enabled, invalid config, feature unavailable, explicit unload |
| Provider-Family Seam | two-adapter proof, behavior characterization, redaction, provider-specific differences |

## Completion rule

A slice is incomplete if any of the following are true:

- coverage is below 100% for an in-scope changed/new module
- an edge case lacks deterministic automated coverage
- a bug fix lacks a regression test
- validation requires real network/key/hardware/screenshot/timing sleep by default
- VS Code Problems show new errors or warnings in touched files
- formatting has not run before tests/checks
