# Feature Specification: Architecture Deepening Plan

**Feature Branch**: `master` (optional Spec Kit branch hook not executed)
**Created**: 2026-05-03
**Status**: Draft
**Input**: User description: "lets make a super comprehensive awesome, bug free, 100% test coverage (yes 100%!!) plan to do all of these things. using spec-kit. go!"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Preserve OCR Session correctness while simplifying ownership (Priority: P1)

As a maintainer of the recording pipeline, I need the OCR Session lifecycle to have one authoritative module interface so that request ownership, cancellation, timeout reuse, failure reporting, and overlay status remain correct while the implementation becomes easier to reason about.

**Why this priority**: OCR Session has a user-visible correctness invariant: OCR results and telemetry must belong only to the user-visible request that started them. If this remains scattered, future changes can create stale OCR results, confusing overlay states, or lost context.

**Independent Test**: Can be fully tested by exercising OCR Session lifecycle scenarios with deterministic fake tasks and verifying state, request ownership, overlay status, and request-log outcomes without real screenshots or network calls.

**Acceptance Scenarios**:

1. **Given** an OCR Session starts for request A and request B starts before A finishes, **When** A returns later, **Then** A's result and telemetry are ignored and B remains the current session.
2. **Given** OCR is awaited with a timeout, **When** the timeout expires before completion, **Then** the session remains running and the result can still be consumed later.
3. **Given** OCR is explicitly cancelled, **When** status is requested or logs are reviewed, **Then** the status and request log consistently report cancellation for the owning request.
4. **Given** OCR fails before provider execution, during capture, or during provider response handling, **When** the user checks the overlay or request log, **Then** a sanitized, request-specific failure reason is available.

---

### User Story 2 - Make settings behavior drift-proof (Priority: P1)

As a maintainer adding or changing a setting, I need defaults, normalization, profile inheritance, and effective settings views to be represented by clear module interfaces so that the UI, persisted settings, and runtime behavior cannot silently drift.

**Why this priority**: Settings currently drive user-visible provider choice, privacy behavior, overlay behavior, output behavior, and pipeline behavior. Drift between default values or inheritance rules can look like data loss or broken preferences.

**Independent Test**: Can be fully tested by loading representative settings snapshots and verifying normalized settings, default values, explicit-null behavior, inherited values, and effective values match expected outcomes.

**Acceptance Scenarios**:

1. **Given** a setting is absent from persisted state, **When** settings are read, **Then** the normalized value matches the canonical default used by runtime behavior.
2. **Given** a setting uses explicit null to mean disabled or inherited, **When** settings are read or migrated, **Then** null is preserved and not overwritten by an unrelated fallback.
3. **Given** a profile overrides a global setting, **When** effective settings are requested for that profile, **Then** the profile value is used and the inherited/global/default source is traceable.
4. **Given** a malformed stored value, **When** settings are normalized, **Then** a safe, documented fallback is used without corrupting valid neighboring settings.

---

### User Story 3 - Make settings runtime sync explicit and complete (Priority: P1)

As a user changing settings, I need settings changes that affect runtime behavior or secondary windows to take effect predictably, without requiring maintainers to remember scattered manual sync calls.

**Why this priority**: A missed runtime sync can make a setting appear saved while the active pipeline or overlay continues using stale behavior.

**Independent Test**: Can be fully tested by applying each settings-change category and verifying the expected runtime sync, secondary-window notification, both, or neither occurs exactly once.

**Acceptance Scenarios**:

1. **Given** a pipeline-affecting setting changes, **When** the change is saved, **Then** runtime pipeline configuration is refreshed before the user starts the next affected action.
2. **Given** a secondary-window-affecting setting changes, **When** the change is saved, **Then** secondary windows receive one settings-change notification with enough context to refresh safely.
3. **Given** a setting is purely historical or display-only, **When** it changes, **Then** no unnecessary pipeline sync is performed.
4. **Given** multiple related settings change together, **When** the batch is saved, **Then** sync and notifications are deduplicated while preserving all semantic effects.

---

### User Story 4 - Make Transcription Flow routing strategy-independent (Priority: P2)

As a maintainer of Transcription Flow, I need preset routing to return one clear routing decision regardless of the underlying strategy so that Transcription Flow does not need to know strategy-specific diagnostics, thresholds, or response shapes.

**Why this priority**: Preset routing is part of Transcription Flow, but strategy-specific details reduce locality and make future routing changes riskier than necessary.

**Independent Test**: Can be fully tested by using deterministic router outcomes and verifying Transcription Flow handles routed preset, default target, no decision, ambiguous decision, and router failure consistently.

**Acceptance Scenarios**:

1. **Given** routing selects a preset, **When** Transcription Flow continues, **Then** the selected preset drives rewrite behavior and request-log preset fields are accurate.
2. **Given** routing selects the default target, **When** Transcription Flow continues, **Then** default-target behavior is explicit and distinguishable from a router failure.
3. **Given** routing is ambiguous or unavailable, **When** Transcription Flow continues, **Then** fallback behavior is deterministic and logged without leaking strategy-specific handling into callers.
4. **Given** a future routing strategy is added, **When** it returns a valid routing decision, **Then** existing Transcription Flow behavior remains unchanged.

---

### User Story 5 - Separate profile matching from effective profile behavior (Priority: P2)

As a maintainer of per-program behavior, I need profile matching, effective preset selection, and Active Window OCR mode resolution to have clear module interfaces so that changing one concern does not risk changing the others.

**Why this priority**: Per-program profiles determine provider selection, preset routing, rewrite behavior, and OCR behavior. Combining unrelated rules in one module makes it harder to review and test changes safely.

**Independent Test**: Can be fully tested by validating program matching cases, default profile behavior, effective preset behavior, and Active Window OCR mode inheritance independently.

**Acceptance Scenarios**:

1. **Given** a foreground program path differs only by case, prefix, or executable suffix, **When** profile matching runs, **Then** the intended profile is selected consistently.
2. **Given** no profile matches the foreground program, **When** effective behavior is requested, **Then** the default profile or global fallback is used according to documented rules.
3. **Given** Active Window OCR mode is configured at profile, default profile, and global levels, **When** rewrite, Quick Ask, or Quick Replace resolves its mode, **Then** the expected precedence is applied for that flow only.
4. **Given** a profile is disabled or malformed, **When** matching and effective behavior are computed, **Then** disabled or invalid data cannot accidentally override safe defaults.

---

### User Story 6 - Localize local-provider lifecycle rules (Priority: P2)

As a maintainer of STT Provider Resolution, I need local-provider load, cache, manual-mode, managed-mode bypass, and provider-readiness rules to live behind a focused module interface so that local providers can evolve without scattering special cases.

**Why this priority**: Local providers are user-visible and performance-sensitive. Scattered lifecycle rules can cause unwanted model loads, stale cache keys, or incorrect provider fallback.

**Independent Test**: Can be fully tested with fake local-provider adapters and deterministic configuration inputs, without loading real models or calling external providers.

**Acceptance Scenarios**:

1. **Given** a local provider is in manual load mode and is not loaded, **When** transcription needs it, **Then** the user receives a clear, safe failure instead of an unexpected blocking load.
2. **Given** a local provider is already loaded for a matching effective configuration, **When** configuration sync occurs, **Then** the loaded provider remains reusable unless a load-affecting value changed.
3. **Given** managed inference is enabled, **When** a local provider is selected, **Then** local-provider traffic is never routed through managed transport.
4. **Given** the local provider configuration changes, **When** cache identity is computed, **Then** stale providers cannot be reused for incompatible settings.

---

### User Story 7 - Provide a safe path for provider-family seams (Priority: P3)

As a maintainer adding or changing provider behavior, I need a documented, testable path for provider-family concerns such as managed-mode adaptation, provider error classification, cost reporting, and request metadata so that repeated provider-specific changes do not remain scattered forever.

**Why this priority**: This area has broad reach and should be addressed carefully after the more immediate high-locality improvements. It still needs a plan so the initiative covers all discovered opportunities.

**Independent Test**: Can be tested by selecting one provider-family concern at a time, proving existing provider behavior remains unchanged, and demonstrating that at least two adapters use the new seam before declaring the seam real.

**Acceptance Scenarios**:

1. **Given** a provider-family concern currently exists in multiple places, **When** it is selected for deepening, **Then** the plan identifies at least two real adapters or defers the seam as hypothetical.
2. **Given** provider error handling changes, **When** representative provider failures occur, **Then** retryability, authentication, quota, timeout, and user-visible failure categories are classified consistently.
3. **Given** provider metadata or cost reporting changes, **When** a request completes or fails, **Then** request logs and cost summaries remain redacted, accurate, and provider-category aware.
4. **Given** a proposed provider-family seam conflicts with an existing decision, **When** the conflict is found, **Then** the plan records whether to honor the decision or explicitly reopen it.

---

### User Story 8 - Prove the initiative is bug-free with 100% in-scope coverage (Priority: P1)

As a release owner, I need the architecture-deepening initiative to be validated with deterministic automated tests and 100% in-scope coverage so that the refactors are safe, reviewable, and regression-resistant.

**Why this priority**: The user explicitly requires a bug-free plan with 100% test coverage. This is the non-negotiable quality gate for every candidate.

**Independent Test**: Can be fully tested by running the agreed validation suite, inspecting coverage reports for every in-scope module and behavior, and verifying no documented edge case lacks an automated check.

**Acceptance Scenarios**:

1. **Given** any in-scope module or module interface is changed, **When** coverage is measured, **Then** that in-scope area reports 100% statement, branch, and function coverage for reachable behavior.
2. **Given** platform-conditional or environment-sensitive behavior exists, **When** the plan reaches implementation, **Then** deterministic adapters or simulations cover the behavior instead of requiring real network, real API keys, real audio hardware, or timing sleeps.
3. **Given** a defect is found during implementation, **When** it is fixed, **Then** a regression test is added before the work is considered complete.
4. **Given** coverage cannot be measured for a path, **When** planning reviews that path, **Then** implementation is blocked until a deterministic coverage strategy or explicit scope decision is recorded.

### Edge Cases

- OCR Session tasks finish after their owning request has ended, after a new request has started, while another caller is awaiting, or after explicit cancellation.
- Persisted settings contain absent keys, explicit null values, malformed values, legacy shapes, policy-enforced values, or partially migrated profile data.
- Multiple settings change in one operation and require different runtime effects.
- Profile matching receives full paths, executable basenames, paths with Windows prefixes, changed install locations, disabled profiles, duplicate matches, or missing foreground app information.
- Transcription Flow receives empty transcript text, router-disabled profiles, no valid routing candidates, ambiguous routing scores, unknown preset ids, provider failures, or cancellation during routing/rewrite.
- Local providers are unavailable, manually unloaded, already loaded under a previous configuration, configured without required model data, or selected while managed inference is enabled.
- Provider-family changes must preserve redaction, privacy posture, request-log accuracy, cost accuracy, and fallback behavior.
- Tests must avoid flakiness from real network calls, API keys, hardware devices, clock sleeps, race-prone background tasks, or unordered logs.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The initiative MUST cover all seven discovered deepening opportunities: OCR Session, settings defaults/views, settings runtime sync, Transcription Flow routing, profile/effective behavior resolution, local-provider lifecycle, and provider-family seams.
- **FR-002**: Each deepening opportunity MUST have a bounded scope, priority, dependencies, acceptance scenarios, and deterministic validation strategy before implementation begins.
- **FR-003**: Existing user-visible behavior MUST be preserved unless a deliberate behavior change is explicitly documented with migration and validation expectations.
- **FR-004**: The OCR Session lifecycle MUST expose one authoritative module interface for request ownership, task state, cancellation, timeout handling, result reuse, failure status, overlay status, and request-log correlation.
- **FR-005**: Settings behavior MUST expose canonical defaults, normalization rules, migration behavior, explicit-null semantics, profile inheritance, and effective settings views through testable module interfaces.
- **FR-006**: Settings runtime sync behavior MUST classify settings changes by their runtime effect and apply the required pipeline refresh and secondary-window notification behavior exactly once per logical change.
- **FR-007**: Transcription Flow MUST consume routing outcomes through a strategy-independent routing decision interface that distinguishes selected preset, default target, no decision, ambiguity, and failure.
- **FR-008**: Profile behavior MUST separate program matching from effective preset selection and Active Window OCR mode resolution.
- **FR-009**: Local-provider lifecycle behavior MUST centralize local load policy, cache identity, manual-mode behavior, managed-transport bypass, readiness validation, and safe failure messaging.
- **FR-010**: Provider-family seam work MUST only introduce a seam when at least two real adapters use it; otherwise the initiative MUST record the seam as hypothetical and defer it.
- **FR-011**: Every changed or newly introduced module interface MUST have deterministic automated tests that exercise normal paths, edge cases, errors, and cancellation/fallback behavior.
- **FR-012**: The initiative MUST achieve 100% statement, branch, and function coverage for every in-scope changed or newly introduced module, including behavior reachable through module interfaces.
- **FR-013**: No default validation path MAY require real network calls, API keys, real audio hardware, screenshots, or timing sleeps.
- **FR-014**: Every bug found during the initiative MUST be fixed with a regression test that fails before the fix and passes after the fix.
- **FR-015**: Any new domain term or renamed architecture concept introduced by the initiative MUST be recorded in the project context before the implementation is considered complete.
- **FR-016**: Any decision to reject, defer, or constrain a deepening opportunity for load-bearing reasons MUST be recorded so future architecture reviews do not rediscover the same rejected path.
- **FR-017**: The final deliverable MUST provide a sequencing plan that allows each opportunity to be implemented, reviewed, tested, and shipped independently where practical.
- **FR-018**: The final deliverable MUST include a rollback or safe-stop strategy for each opportunity so partial completion does not leave the codebase in an inconsistent state.
- **FR-019**: The final validation evidence MUST list each in-scope module, the behaviors tested, the coverage result, and any remaining risk accepted by the user.

### Key Entities *(include if feature involves data)*

- **Deepening Opportunity**: A candidate refactor area with scope, priority, dependencies, expected locality gain, expected leverage gain, acceptance scenarios, and validation evidence.
- **Module Interface**: The complete surface a caller must understand to use a module correctly, including invariants, ordering constraints, error modes, configuration expectations, and observable outcomes.
- **OCR Session**: The request-owned active-window OCR lifecycle whose results and telemetry must remain tied to the session that started them.
- **Settings View**: A normalized representation of persisted settings, defaults, explicit-null semantics, profile inheritance, and effective runtime values.
- **Runtime Sync Policy**: A classification of settings changes by whether they require runtime pipeline refresh, secondary-window notification, both, or neither.
- **Routing Decision**: A strategy-independent Transcription Flow outcome describing preset selection, default target selection, no decision, ambiguity, or failure plus diagnostics.
- **Profile Resolution**: The process of selecting a program profile and deriving effective preset and Active Window OCR behavior without mixing unrelated matching rules.
- **Local Provider Lifecycle**: Local-provider readiness, cache identity, load/unload behavior, managed-transport bypass, and safe user-visible failure states.
- **Provider-Family Seam**: A shared module interface for repeated provider concerns that is considered real only when at least two adapters satisfy it.
- **Coverage Gate**: The validation requirement that all in-scope changed or introduced module behavior has 100% statement, branch, and function coverage plus deterministic edge-case tests.

## Constitution & Risk Notes *(mandatory)*

- **Sensitive data touched**: Audio, transcripts, OCR text, prompts, provider responses, settings, API-key presence checks, auth/session posture, policy data, request logs, and cost/usage metadata may be affected indirectly. Secrets must never be logged, test fixtures must avoid real sensitive data, and request-log redaction must be preserved.
- **External services / network use**: No new default validation path may require external services. Provider-related behavior must be tested with fake adapters, fixtures, or deterministic simulations unless a manual ignored validation path is explicitly documented.
- **Settings / persisted state**: Settings defaults, migrations, normalization, explicit-null behavior, profile inheritance, and runtime sync policy are in scope. Backward compatibility with existing settings snapshots is required.
- **Contract surfaces**: Settings-change notifications, runtime sync behavior, request-log fields, generated schemas/types, provider identifiers, routing outcomes, and pipeline status semantics may be affected and must remain contract-tested where applicable.
- **Pipeline / background work**: Recording, transcription, cancellation, OCR tasks, routing, rewrite, local-provider load behavior, and secondary-window refresh behavior are in scope. Race-prone behavior must be controlled by deterministic tests rather than sleeps.
- **Deterministic validation approach**: Use fake providers, fake tasks, fake settings snapshots, controlled cancellation, fixture-based request logs, and module-interface tests. Coverage evidence must be produced for every changed or newly introduced in-scope module.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of in-scope changed or newly introduced modules report statement, branch, and function coverage for reachable behavior before completion.
- **SC-002**: 100% of documented edge cases in this specification have at least one deterministic automated test or are blocked from implementation until such a test can be created.
- **SC-003**: 0 default validation tests require real network calls, API keys, real audio hardware, screenshots, or timing sleeps.
- **SC-004**: 0 known defects remain open in the in-scope areas at handoff; every defect found during implementation has a regression test.
- **SC-005**: 100% of the seven deepening opportunities have documented scope, dependencies, sequencing, acceptance checks, rollback/safe-stop guidance, and coverage evidence.
- **SC-006**: Existing behavior characterization tests pass for all affected recording, transcription, OCR, settings, profile, provider, logging, and overlay flows.
- **SC-007**: Settings default and effective-value drift checks cover every in-scope setting touched by the initiative.
- **SC-008**: Runtime sync policy tests cover every in-scope setting-change category and verify no duplicate sync/notification occurs for a single logical change.
- **SC-009**: Routing decision tests cover selected preset, default target, no decision, ambiguity, failure, and cancellation outcomes.
- **SC-010**: Local-provider lifecycle tests cover unloaded, manually blocked, loaded, configuration-changed, managed-enabled, and invalid-configuration states.

## Assumptions

- The initiative is scoped to the seven opportunities surfaced in the architecture review unless the user explicitly adds or removes scope during planning.
- "Bug-free" means zero known defects at handoff, all acceptance scenarios passing, regression tests for every discovered defect, and no untested documented edge case.
- "100% test coverage" applies to every changed or newly introduced in-scope module and every behavior reachable through the module interfaces in this initiative.
- Existing public behavior should be preserved by default; any intentional behavior change requires explicit documentation and validation evidence.
- Provider-family seam work should be sequenced after the higher-priority OCR, settings, routing, profile, and local-provider work unless planning reveals a dependency.
- The plan may use fake adapters and deterministic simulations to cover environment-sensitive behavior, but default validation must not depend on live services or machine-specific devices.
- Documentation updates are part of the work when the initiative names a new domain concept, changes a known invariant, or records a load-bearing rejection/defer decision.
