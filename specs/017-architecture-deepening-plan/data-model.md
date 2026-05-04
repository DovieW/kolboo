# Phase 1 Data Model: Architecture Deepening Plan

**Feature**: 017 Architecture Deepening Plan
**Date**: 2026-05-03

This data model defines planning entities and target module-interface concepts. It intentionally describes behavior and relationships without locking implementation to specific types.

## Entity: Deepening Opportunity

**Represents**: One architecture refactor candidate that turns a shallow or leaky module area into a deeper module with better locality and leverage.

**Fields**:

- `id`: Stable opportunity identifier.
- `name`: Human-readable name.
- `priority`: P1, P2, or P3.
- `domain_area`: OCR Session, settings, Transcription Flow, profiles, local providers, or provider family.
- `problem_statement`: Current locality/leverage friction.
- `target_depth`: What behavior should move behind a smaller interface.
- `scope`: Included files/modules and explicit exclusions.
- `dependencies`: Other opportunities or guardrails that must happen first.
- `acceptance_scenarios`: User-visible or maintainer-visible behavior checks from the spec.
- `coverage_requirements`: Required statement, branch, function, edge-case, and regression coverage.
- `rollback_strategy`: Safe-stop or revert strategy if the slice cannot complete.
- `status`: Planned, in progress, complete, deferred, or rejected.

**Validation rules**:

- Must map to at least one functional requirement.
- Must have deterministic validation before implementation starts.
- Must not introduce an external seam unless at least two real adapters are identified, except when explicitly marked hypothetical/deferred.

**Relationships**:

- Has many `ModuleInterfaceContract` entries.
- Has many `ValidationEvidence` entries.
- May produce one or more `DecisionRecord` entries.

## Entity: Module Interface Contract

**Represents**: The complete caller-facing behavior for a deepened module seam.

**Fields**:

- `module_name`: Name of the module seam.
- `callers`: Known callers crossing the seam.
- `responsibilities`: Behavior hidden behind the interface.
- `non_responsibilities`: Behavior deliberately left outside the seam.
- `invariants`: Rules callers may rely on.
- `ordering_constraints`: Required sequencing, if any.
- `error_modes`: Expected failures and how callers observe them.
- `observability`: Logs, diagnostics, events, or request-log fields produced.
- `test_surface`: The behaviors that must be tested through this interface.

**Validation rules**:

- Must describe interface behavior beyond type signatures.
- Must identify fake adapters or deterministic fixtures for tests.
- Must include privacy/redaction expectations when sensitive data can cross the seam.

**Relationships**:

- Belongs to one `DeepeningOpportunity`.
- May be satisfied by multiple `Adapter` entities.

## Entity: OCR Session State

**Represents**: Request-owned active-window OCR lifecycle state and outcomes.

**Fields**:

- `session_id`: User-visible request identifier used for ownership.
- `task_state`: Not started, running, awaiting, done, failed, cancelled, or stale.
- `task_owner`: The request/session associated with the active task.
- `abort_handle_state`: Whether cancellation can still reach the active task.
- `result`: Sanitized OCR result text/provider/model when successful.
- `failed_reason`: Sanitized failure reason when failed.
- `cancelled`: Whether explicit cancellation occurred.
- `request_log_id`: Request log entry that receives OCR telemetry.

**Validation rules**:

- A result or failure may only update state if its task owner matches the current session.
- Timeout while awaiting must not make a running task unobservable or unconsumable.
- Explicit cancellation must clear reusable result state and mark the owning request cancelled.
- Failure reasons exposed to overlay must be sanitized and bounded.

**State transitions**:

- `not_started -> running` when a validated OCR task begins.
- `running -> awaiting` when a consumer temporarily awaits the task.
- `awaiting -> running` when await times out and the task is restored.
- `running|awaiting -> done` when matching task succeeds.
- `running|awaiting -> failed` when matching task fails.
- `running|awaiting|done|failed -> cancelled` on explicit cancellation.
- Any non-current task outcome becomes `stale` and must not mutate current session state.

## Entity: Settings View

**Represents**: Normalized settings plus source-aware defaults and effective values.

**Fields**:

- `setting_key`: Stable settings key.
- `raw_value`: Persisted value before normalization.
- `normalized_value`: Safe value after migration/normalization.
- `default_value`: Canonical fallback value.
- `source`: Explicit value, explicit null, inherited value, default, migrated, or policy-enforced.
- `scope`: Global, profile, preset, runtime-effective, or UI-only.
- `sensitive`: Whether the value can expose sensitive user data or provider state.
- `runtime_effect`: None, pipeline, secondary window, or both.

**Validation rules**:

- Missing and invalid values may fall back to defaults.
- Explicit null must be preserved when null means disabled or inherited.
- Profile and preset values must retain inheritance semantics.
- Runtime-effective views must be traceable to their source.

**Relationships**:

- Used by `Runtime Sync Policy`.
- Used by profile/effective behavior resolution.

## Entity: Runtime Sync Policy

**Represents**: The runtime side effects required after settings change.

**Fields**:

- `setting_key`: Stable settings key or key group.
- `change_kind`: Single setting update, batch update, policy normalization, migration, API-key change, or profile update.
- `pipeline_sync_required`: Whether runtime pipeline config must refresh.
- `settings_changed_event_required`: Whether secondary windows must refresh cached settings.
- `payload_shape`: Minimal semantic payload, if any.
- `dedupe_key`: Logical change identity used to avoid duplicate sync/events.

**Validation rules**:

- One logical change may cause at most one pipeline sync and one secondary-window notification.
- Pipeline-affecting changes must sync before the next affected user action.
- Overlay-affecting changes must notify secondary windows.
- Non-runtime changes must not perform unnecessary sync.

## Entity: Routing Decision

**Represents**: Strategy-independent preset-routing outcome for Transcription Flow.

**Fields**:

- `outcome`: Selected preset, default target, no decision, ambiguous, failed, or cancelled.
- `selected_preset_id`: Present only for selected preset outcome.
- `strategy`: Embeddings, LLM, or future strategy identifier.
- `diagnostics`: Structured request/response/scores/timing diagnostics.
- `fallback_reason`: Why fallback/default behavior is used.
- `request_log_patch`: Request-log updates derived from the decision.

**Validation rules**:

- Default target and no decision must be distinguishable.
- Unknown preset ids must not be treated as valid selections.
- Diagnostics must be redacted and bounded.
- Cancellation must remain higher priority than provider/strategy failure.

## Entity: Profile Resolution

**Represents**: Profile-related decisions separated into matching and effective behavior.

**Fields**:

- `foreground_identity`: Foreground app path or unavailable state.
- `matched_profile_id`: Profile selected by program path matching, if any.
- `default_profile_id`: Default profile fallback, if any.
- `effective_preset_id`: Preset selected by active/default rules, if any.
- `active_window_ocr_modes`: Effective modes for rewrite, Quick Ask, and Quick Replace.
- `disabled_or_invalid_reason`: Reason a profile or value could not participate.

**Validation rules**:

- Program matching must be deterministic across path case, slash style, Windows prefixes, basename-only configuration, and executable suffix differences.
- Effective behavior must use documented precedence.
- Disabled or invalid profiles must not override safe defaults.

## Entity: Local Provider Lifecycle

**Represents**: Local STT provider load, cache, readiness, and transport behavior.

**Fields**:

- `provider_id`: Local provider identifier.
- `cache_identity`: Configuration-derived identity for reuse safety.
- `load_mode`: Manual, on transcribe, on launch, or provider-specific equivalent.
- `readiness`: Unconfigured, unloaded, loading, loaded, failed, or unavailable.
- `managed_transport_allowed`: Always false for local providers.
- `safe_failure_message`: User-facing failure when provider cannot run.
- `eviction_reason`: Configuration change, explicit unload, feature unavailable, or memory pressure.

**Validation rules**:

- Manual mode must not unexpectedly load heavy local resources.
- Loaded providers may only be reused for compatible cache identity.
- Managed inference must never route local provider traffic.
- Missing configuration must fail safely and clearly.

## Entity: Provider-Family Seam Candidate

**Represents**: A possible shared seam for repeated provider concerns.

**Fields**:

- `concern`: Managed adaptation, error classification, cost reporting, request metadata, or other provider-family behavior.
- `adapters`: Real provider adapters expected to satisfy the seam.
- `callers`: Current duplicated callers that would gain leverage.
- `locality_gain`: What maintenance knowledge moves behind the seam.
- `hypothetical`: True if fewer than two adapters exist.
- `decision`: Implement, defer, reject, or reopen decision.

**Validation rules**:

- Must identify at least two real adapters before implementation.
- Must preserve provider-specific behavior and request-log privacy.
- Must not introduce pass-through interfaces that fail the deletion test.

## Entity: Coverage Gate

**Represents**: The evidence required to claim a slice is complete.

**Fields**:

- `scope`: Changed/new in-scope modules.
- `coverage_metric`: Statement, branch, function, and edge-case coverage.
- `required_percent`: 100 for in-scope reachable behavior.
- `coverage_tool`: Vitest V8 for TypeScript; planned Rust coverage tool for Rust.
- `edge_case_matrix`: Mapping from spec edge cases to tests.
- `regression_tests`: Tests added for defects found during implementation.
- `manual_exclusions`: Ignored/manual checks with rationale and instructions.

**Validation rules**:

- No changed/new in-scope module can complete without coverage evidence.
- Exclusions require explicit scope decision and cannot hide reachable behavior.
- Manual tests cannot replace deterministic default validation.
