# Phase 0 Research: Architecture Deepening Plan

**Feature**: 017 Architecture Deepening Plan
**Date**: 2026-05-03
**Status**: Complete — no unresolved clarifications

## Research Scope

This research resolves planning decisions for the seven deepening opportunities and the explicit 100% in-scope coverage requirement. It is grounded in:

- `CONTEXT.md` domain terms: STT Provider Resolution, STT Execution, Transcription Flow, OCR Session.
- `.specify/memory/constitution.md` requirements for privacy, deterministic validation, contract sync, settings sync, pipeline safety, and tooling.
- Existing repo scripts in `app/package.json` and coverage setup in `app/vite.config.ts`.
- Current code structure under `app/src/**` and `app/src-tauri/src/**`.

## Decision 1: Treat this as a staged architecture initiative, not one monolithic refactor

**Decision**: Implement the seven opportunities as independently reviewable slices with shared guardrails and coverage reporting, sequenced from highest correctness risk to broadest provider-family exploration.

**Rationale**:

- OCR Session, settings behavior, and runtime sync are P1 because they directly affect user-visible correctness and trust.
- Routing, profile resolution, and local-provider lifecycle are P2 because they improve locality and reduce future regression risk after the P1 safety seams are established.
- Provider-family seams are P3 because they span many providers and must satisfy the "two adapters = real seam" rule before introducing shared interfaces.
- Staging preserves reviewability and provides rollback/safe-stop points between opportunities.

**Alternatives considered**:

- **Single sweeping PR**: rejected because it would make behavior preservation and coverage evidence hard to review.
- **Only document opportunities without implementation sequencing**: rejected because the user explicitly requested a comprehensive Spec Kit plan.
- **Implement provider-family seams first**: rejected because those seams have the broadest blast radius and depend on better coverage discipline.

## Decision 2: Scope 100% coverage to changed or newly introduced in-scope modules and reachable behavior

**Decision**: The coverage gate requires 100% statement, branch, and function coverage for every changed or newly introduced in-scope module and every behavior reachable through the module interfaces created by this initiative.

**Rationale**:

- The repo does not currently enforce 100% global coverage across all existing modules.
- Global 100% coverage for all existing untouched code would turn the architecture initiative into an unbounded historical test rewrite.
- In-scope coverage is strict enough to satisfy the quality intent while keeping the plan executable.
- The spec already defines that coverage gaps block implementation unless a deterministic coverage strategy or explicit scope decision is recorded.

**Alternatives considered**:

- **100% global repository coverage**: rejected as out of scope and likely to drown the architecture work in unrelated legacy coverage.
- **Only existing thresholds**: rejected because the user explicitly requested 100% coverage for this work.
- **Statement-only coverage**: rejected because branch behavior is critical for cancellation, fallback, inheritance, and provider lifecycle paths.

## Decision 3: Use existing validation commands plus add a Rust coverage strategy before implementation starts

**Decision**: Use existing commands for formatting, linting, typechecking, tests, schema checks, and CI gate; require implementation tasks to add or document a Rust coverage command before Rust refactor completion.

Existing verified scripts:

- TypeScript formatting/linting: `pnpm -C app lint` and `pnpm -C app lint:ci`
- TypeScript tests: `pnpm -C app test`
- TypeScript coverage: `pnpm -C app coverage`
- Rust formatting: `pnpm -C app cargo:fmt`
- Rust tests: `pnpm -C app cargo:test`
- Cross-cutting tests: `pnpm -C app test:all`
- Final gate: `pnpm -C app check:ci`

Coverage decision:

- TypeScript coverage uses existing Vitest V8 coverage.
- Rust coverage is not currently exposed by package scripts; the implementation plan must add an explicit, documented Rust coverage path for in-scope modules, preferably via `cargo llvm-cov` or an equivalent deterministic Rust coverage tool, before claiming 100% Rust in-scope coverage.

**Rationale**:

- The constitution requires smallest validating command first, then final CI gate.
- P1/P2 work touches Rust and TypeScript, so test commands must cover both sides.
- Rust module-interface coverage cannot be credibly claimed from `cargo test` alone.

**Alternatives considered**:

- **Skip Rust coverage measurement**: rejected because the spec explicitly requires coverage evidence.
- **Use only manual inspection for Rust branch coverage**: rejected because the success criteria require measurable outcomes.
- **Immediately add a new dependency during planning**: rejected because this phase is plan/design only; the task phase should make the tooling change deliberately.

## Decision 4: No default validation may use real providers, keys, audio hardware, screenshots, or timing sleeps

**Decision**: All default tests must use fake providers, fake tasks, fixture settings snapshots, controlled cancellation, and direct state manipulation. Manual tests that use real hardware or providers must be ignored/manual and documented separately.

**Rationale**:

- This is required by the constitution and by the spec.
- The deepening opportunities are mostly module-interface refactors; correctness can be proven through deterministic seams.
- Race-prone workflows such as OCR task timeout and cancellation must be controlled with fake task handles or deterministic channels, not sleeps.

**Alternatives considered**:

- **Use local provider or OCR smoke tests as default validation**: rejected because they require machine-specific setup.
- **Use short sleeps for async tasks**: rejected as flaky and explicitly forbidden by the constitution.
- **Use live provider mocks over localhost HTTP for all provider tests**: acceptable only when deterministic and local, but fake adapters are preferred for module-interface tests.

## Decision 5: OCR Session should be the first implementation slice

**Decision**: Sequence OCR Session before settings/routing/provider work.

**Rationale**:

- It has the tightest correctness invariant in `CONTEXT.md`: task results and telemetry belong only to the session that started them.
- The current state is visibly scattered across `PipelineInner` fields while behavior is already partially localized in `pipeline/ocr_session.rs`.
- It provides a good model for deepening: small external interface, more implementation hidden behind it, deterministic lifecycle tests.

**Alternatives considered**:

- **Start with settings defaults**: reasonable, but settings work spans TS and Rust and is broader.
- **Start with routing**: lower immediate correctness risk than OCR Session.
- **Start with local providers**: depends on better coverage discipline and cache/lifecycle characterization.

## Decision 6: Settings work should split into Defaults/View first, Runtime Sync second

**Decision**: Treat settings defaults/effective views and runtime sync as two related but separate slices.

**Rationale**:

- Settings defaults and effective settings views answer "what does this value mean?"
- Runtime sync answers "what runtime effects should this change cause?"
- Combining both at once would create a large mixed seam and make tests harder to localize.
- The defaults/view slice provides the vocabulary and keys needed by the runtime sync policy.

**Alternatives considered**:

- **One settings mega-module**: rejected because it risks becoming another shallow mixed module.
- **Runtime sync first**: rejected because sync classifications depend on clear setting keys and effective semantics.
- **Only centralize constants**: rejected because the real friction includes normalization, explicit null, migration, and inheritance semantics.

## Decision 7: Routing should return a first-class Routing Decision with diagnostics separated from selection

**Decision**: The routing slice should make Transcription Flow consume a strategy-independent Routing Decision that separates selection outcome from diagnostics/logging payload.

**Rationale**:

- Current embeddings and LLM router functions return different shapes.
- Transcription Flow should know whether the result is selected preset, default target, no decision, ambiguity, failure, or cancellation, not how the strategy produced it.
- Diagnostics still matter for request logs, but should be carried as structured data rather than tuple-specific caller knowledge.

**Alternatives considered**:

- **Keep strategy-specific functions and only wrap tuples**: rejected because it preserves caller knowledge of strategy details.
- **Hide all diagnostics**: rejected because request logs are a user/debugging contract.
- **Add a trait immediately for all routers**: defer exact interface shape to implementation tasks, but plan requires a decision entity and adapter-compatible structure.

## Decision 8: Profile matching and effective behavior should be separate modules

**Decision**: Split profile matching from effective profile behavior resolution, including effective preset and Active Window OCR mode.

**Rationale**:

- Program path matching has different inputs, error modes, and tests than OCR mode inheritance.
- Active Window OCR mode has flow-specific precedence for rewrite, Quick Ask, and Quick Replace.
- Separating the modules increases locality and lets future changes avoid unrelated regression risk.

**Alternatives considered**:

- **Keep all helpers in one profile module**: rejected because the current module already mixes concerns.
- **Move all profile behavior to settings**: rejected because backend runtime behavior still needs Rust-side deterministic resolution.
- **Share TS/Rust logic by code generation immediately**: deferred; fixtures and contract tests can keep behavior aligned first.

## Decision 9: Local-provider lifecycle should be localized before broader provider-family seams

**Decision**: Address local-provider lifecycle as a focused slice before managed-mode/error/cost/metadata provider-family seams.

**Rationale**:

- Local Whisper and Whisper Server have special cache, load, and managed-bypass behavior today.
- Local-provider behavior is concrete and already has multiple call sites.
- Provider-family seams are broader and must prove two real adapters before introducing shared interfaces.

**Alternatives considered**:

- **Fold local providers into one generic provider lifecycle immediately**: rejected because cloud, managed, and local provider lifecycles vary enough that the deeper local seam should be proven first.
- **Leave local provider special cases inline**: rejected because it violates the locality goal and increases cache/load bug risk.
- **Create a provider-family registry first**: rejected as too broad for this slice.

## Decision 10: Provider-family seams require a pre-flight decision record per concern

**Decision**: For provider-family managed mode, error classification, cost reporting, and request metadata, implementation must first record whether the seam is real by identifying at least two adapters and expected caller leverage.

**Rationale**:

- The architecture skill requires "one adapter = hypothetical seam; two adapters = real seam."
- Provider-family work can easily create shallow abstractions if introduced too early.
- A pre-flight decision prevents re-litigating broad abstractions without evidence.

**Alternatives considered**:

- **Implement all provider-family seams in one pass**: rejected due to broad blast radius.
- **Defer provider-family seams entirely**: rejected because the spec explicitly includes them.
- **Introduce interfaces based on expected future providers**: rejected as hypothetical seam design.

## Decision 11: Contract artifacts should document module-interface contracts, not HTTP/OpenAPI contracts

**Decision**: Use markdown module-interface contracts in `contracts/` because this feature mostly deepens internal module seams and UI/backend behavior contracts rather than exposing a new web API.

**Rationale**:

- The project is a desktop app with Rust/Tauri backend and React UI.
- Relevant interfaces are module seams, settings/event contracts, and validation gates.
- OpenAPI would be misleading for this feature.

**Alternatives considered**:

- **Skip contracts**: rejected because the feature changes important internal and UI/backend contracts.
- **OpenAPI contracts**: rejected because no HTTP API is introduced.
- **Only data model**: rejected because module-interface expectations need a stable planning artifact.
