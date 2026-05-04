# Implementation Plan: Architecture Deepening Plan

**Branch**: `master` | **Date**: 2026-05-03 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `specs/017-architecture-deepening-plan/spec.md`

## Summary

Create a staged, test-first architecture-deepening initiative for all seven discovered opportunities: OCR Session ownership, settings defaults/views, settings runtime sync, Transcription Flow routing, profile/effective behavior resolution, local-provider lifecycle, and provider-family seams. The approach is to define module-interface contracts, implement each opportunity as an independently reviewable slice, preserve existing user-visible behavior by default, and enforce a 100% in-scope coverage gate for every changed or newly introduced module.

The plan deliberately sequences high-locality, high-correctness work before broad provider-family seams:

1. OCR Session state/interface deepening.
2. Settings defaults and Settings View.
3. Settings Runtime Sync Policy.
4. Transcription Flow Routing Decision.
5. Profile matching and effective profile behavior split.
6. Local Provider Lifecycle.
7. Provider-Family Seam pre-flight decisions and selected real seams.

## Technical Context

**Language/Version**: TypeScript 5.9.3 with React 19.2.3/Vite 7.3.1; Rust 2021 edition Tauri backend; Node >=24; pnpm 10.26.2.
**Primary Dependencies**: Tauri 2.9.x, `@tauri-apps/api` 2.9.x, `@tauri-apps/plugin-store` 2.4.x, TanStack Query 5.90.x, Mantine 8.3.x, Vitest 4.0.x, Biome 2.3.x, Tokio 1.48, reqwest 0.13, schemars, keyring, cpal/hound, provider-specific STT/LLM modules.
**Storage**: Local `settings.json` via Tauri store for non-secret settings/cache; OS secure storage for API keys/session secrets via backend secrets module; request logs/history/recordings/stats under app data; no new persistent storage format required by the plan unless a slice explicitly changes settings/contracts.
**Testing**: Vitest for TypeScript; `pnpm -C app coverage` for TypeScript coverage; Rust unit/integration tests via `pnpm -C app cargo:test`; final CI gate via `pnpm -C app check:ci`; Rust coverage command must be added or documented before Rust slices claim the 100% in-scope coverage gate.
**Target Platform**: Desktop app, with Windows as the active development/validation environment; codebase also contains macOS/Linux conditional paths that must remain deterministic and feature-gated where applicable.
**Project Type**: Cross-cutting desktop app refactor plan spanning React/Vite UI, Tauri command/event/settings contracts, and Rust backend pipeline/provider modules.
**Performance Goals**: Preserve existing recording/transcription responsiveness; avoid additional local-provider model loads during routine config sync; avoid duplicate runtime sync/events for single logical settings changes; keep request-log/overlay updates bounded and redacted.
**Constraints**: No default validation may require real network calls, API keys, real audio hardware, screenshots, timing sleeps, or user interaction; maintain existing user-visible behavior unless explicitly documented; keep diffs reviewable and slices independently testable; Rust/Cargo local runs must use `sccache` when available and conservative build jobs.
**Scale/Scope**: Seven deepening opportunities, expected to touch `app/src/**`, `app/src-tauri/src/**`, generated contract checks where applicable, tests, docs/refactor notes, and Spec Kit artifacts. Scope is limited to changed/new in-scope modules, not global historical 100% coverage for untouched code.

## Constitution Check

_GATE: Must pass before Phase 0 research. Re-check after Phase 1 design._

- **Privacy/secrets**: PASS — The spec classifies audio, transcripts, OCR text, prompts, provider responses, settings, API-key presence, auth/session posture, policy data, request logs, and cost/usage metadata as sensitive. Plan requires redaction preservation and no secret logging. No new secret storage path is planned.
- **Deterministic validation**: PASS — Default validation is required to use fake providers/tasks, fixture settings snapshots, controlled cancellation, and local request-log fixtures. Real network, keys, audio hardware, screenshots, and sleeps are explicitly disallowed by the spec and contracts.
- **Contract sync**: PASS — The plan identifies settings-change notifications, runtime sync behavior, request-log fields, generated schemas/types, provider identifiers, routing outcomes, and pipeline status semantics as contract surfaces. Affected slices must update generated files/checks when contract surfaces change.
- **Settings/migrations/runtime sync**: PASS — Settings defaults/views and runtime sync are P1 slices with explicit requirements for Rust seeding/migrations, TS normalization/migrations, explicit-null semantics, persistence, pipeline sync, and settings-change notifications.
- **Pipeline safety**: PASS — OCR Session, Transcription Flow routing, local-provider lifecycle, cancellation, background task ownership, and state-machine behavior are explicitly covered by module-interface contracts and deterministic tests.
- **Tooling/review**: PASS — The plan uses smallest relevant validation first, requires formatting before tests/checks, requires coverage evidence, and reserves `pnpm -C app check:ci` for final validation. No constitution violation requires Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/017-architecture-deepening-plan/
├── plan.md                         # This implementation plan
├── research.md                     # Phase 0 decisions and rationale
├── data-model.md                   # Phase 1 planning entities and target concepts
├── quickstart.md                   # Phase 1 validation/use instructions
├── contracts/
│   ├── module-interface-contracts.md
│   └── validation-contract.md
└── tasks.md                        # Created later by /speckit.tasks
```

### Source Code (repository root)

```text
app/
├── src/
│   ├── components/settings/        # Settings UI call sites affected by settings views/sync
│   ├── lib/
│   │   ├── tauri/                  # settings, commands, events, generated types
│   │   ├── queries.ts              # settings mutations and runtime sync call sites
│   │   └── **/*.test.ts(x)         # Vitest tests beside changed TS modules
│   └── overlay/                    # Secondary-window consumers of settings/pipeline state
├── vite.config.ts                  # TypeScript coverage thresholds/gate configuration
├── package.json                    # validation scripts and any added coverage command
└── src-tauri/
    ├── Cargo.toml                  # Rust test/coverage tooling dependencies if needed
    ├── gen/                        # generated schemas/types if contracts change
    └── src/
        ├── pipeline.rs             # current pipeline orchestration and state owner
        ├── pipeline/               # OCR Session, routing, profile, STT provider resolution modules
        ├── settings/               # Rust seeding/migrations/defaults
        ├── stt/                    # STT provider adapters and local-provider behavior
        ├── llm/                    # LLM provider adapters when provider-family seams are selected
        ├── cost/                   # Provider cost behavior when provider-family seams are selected
        ├── request_log.rs          # Request-log metadata/redaction behavior
        └── tests/                  # Broader Rust tests where inline tests are not enough

docs/
├── Refactors/                      # Update when out-of-scope refactors or deferred seams are found
└── User Docs/ or How Tos/          # Update only if user-visible behavior/settings change

.github/
└── copilot-instructions.md         # Plan pointer between SPECKIT markers
```

**Structure Decision**: Use the existing React/Tauri split. Place new TypeScript module-interface tests beside changed TS modules. Place Rust unit tests inline for focused module behavior and broader tests under `app/src-tauri/src/tests/**` only when the behavior spans modules. Keep contracts as markdown because this feature changes internal module seams and UI/backend behavior contracts, not an HTTP API.

## Phase 0: Research

**Status**: Complete — see [research.md](./research.md).

Key decisions:

- Stage the initiative as independently reviewable slices, not one monolithic refactor.
- Scope 100% coverage to changed/new in-scope modules and reachable behavior.
- Use existing validation commands and add/document a Rust coverage command before Rust slices claim coverage completion.
- Keep all default validation deterministic; no real network/keys/audio/screenshot/timing sleeps.
- Start with OCR Session, then settings defaults/views, then runtime sync.
- Treat provider-family seams as pre-flight decisions requiring at least two real adapters.

## Phase 1: Design and Contracts

**Status**: Complete.

Generated artifacts:

- [data-model.md](./data-model.md) — planning entities and target module-interface concepts.
- [contracts/module-interface-contracts.md](./contracts/module-interface-contracts.md) — caller-facing contracts for each deepening opportunity.
- [contracts/validation-contract.md](./contracts/validation-contract.md) — 100% in-scope coverage and deterministic validation contract.
- [quickstart.md](./quickstart.md) — implementation order, coverage gate, deterministic validation rules, and command guidance.

## Post-Design Constitution Check

- **Privacy/secrets**: PASS — Contracts require redaction and privacy preservation for OCR, routing diagnostics, provider-family metadata, request logs, and settings. No new secret storage is introduced by design.
- **Deterministic validation**: PASS — Validation contract explicitly bans real network, keys, paid accounts, audio hardware, screenshots, timing sleeps, and user interaction from default tests.
- **Contract sync**: PASS — Module-interface contracts identify Tauri settings events/sync, request logs, generated schemas/types, provider ids, and routing outcomes as contract surfaces that must be kept synchronized when touched.
- **Settings/migrations/runtime sync**: PASS — Settings View and Runtime Sync Policy contracts cover missing/invalid/default values, explicit null, profile inheritance, policy/license/API-key changes, pipeline sync, and secondary-window notifications.
- **Pipeline safety**: PASS — OCR Session, Routing Decision, Profile Resolution, and Local Provider Lifecycle contracts include state/cancellation/fallback/ownership invariants and tests.
- **Tooling/review**: PASS — Quickstart defines smallest validation commands and final gate; Rust coverage gap is explicitly converted into a pre-completion implementation task, not hidden.

## Complexity Tracking

No constitution violations are present. No justified complexity exceptions are required at this stage.

| Violation | Why Needed | Simpler Alternative Rejected Because |
| --------- | ---------- | ------------------------------------ |
| N/A       | N/A        | N/A                                  |

## Implementation Sequencing

1. **Coverage tooling foundation**: Add or document Rust coverage command; confirm TypeScript per-slice threshold strategy; create coverage evidence template.
2. **OCR Session slice**: Deepen OCR Session state/interface and cover stale/timeout/cancel/failure/reuse behavior.
3. **Settings View slice**: Deepen defaults, normalization, explicit-null, source-aware effective values, and drift tests.
4. **Runtime Sync Policy slice**: Centralize settings runtime side effects and dedupe tests.
5. **Routing Decision slice**: Introduce strategy-independent routing outcome and update Transcription Flow tests.
6. **Profile Resolution slice**: Separate program matching from effective preset/OCR mode behavior; add precedence matrix tests.
7. **Local Provider Lifecycle slice**: Centralize local-provider cache/load/readiness/managed-bypass behavior and tests.
8. **Provider-Family Seam slice**: For each selected concern, record two-adapter proof before implementation; defer hypothetical seams.
9. **Final validation**: Run format before tests, smallest relevant commands per slice, coverage evidence, generated checks when contracts change, and one final `pnpm -C app check:ci`.

## Risk and Rollback Strategy

- **OCR Session**: Safe stop after introducing state wrapper and characterization tests; rollback by keeping existing `SharedPipeline` methods delegating to previous behavior until tests pass.
- **Settings View**: Safe stop after read-only normalization/default module and drift tests; do not switch write paths until parity is proven.
- **Runtime Sync Policy**: Safe stop after policy table tests; migrate call sites incrementally and verify no duplicate sync/events.
- **Routing Decision**: Safe stop by adapting existing router functions to return the new decision while preserving old diagnostics; switch Transcription Flow only after characterization tests pass.
- **Profile Resolution**: Safe stop by extracting matching and effective behavior without changing call order; compare old/new outputs through fixtures.
- **Local Provider Lifecycle**: Safe stop by wrapping existing cache/load functions first; only consolidate behavior after cache identity tests pass.
- **Provider-Family Seam**: Safe stop by recording defer/reject decisions when two-adapter proof is absent; do not introduce pass-through seams.
