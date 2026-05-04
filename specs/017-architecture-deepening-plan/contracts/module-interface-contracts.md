# Contract: Module Interfaces for Architecture Deepening

**Feature**: 017 Architecture Deepening Plan
**Status**: Draft contract for implementation tasks

This contract describes the expected module-interface behavior for each deepening opportunity. It is not an HTTP contract; it is the testable caller-facing contract for internal seams and UI/backend behavior.

## Common contract rules

Every deepened module interface MUST:

1. Hide implementation detail behind a smaller caller-facing surface.
2. Document invariants, ordering constraints, error modes, observability, and test surface.
3. Preserve existing user-visible behavior unless a deliberate behavior change is documented.
4. Provide deterministic fake adapters or fixtures for tests.
5. Meet the in-scope 100% coverage gate before the slice is complete.
6. Avoid real network calls, real API keys, real audio hardware, screenshots, and timing sleeps in default validation.

## OCR Session interface contract

### Callers

- Recording pipeline lifecycle operations.
- OCR task orchestration.
- Overlay status readers.
- Request-log correlation code.
- Quick Ask / Quick Replace / rewrite flows that may consume OCR context.

### Responsibilities

- Own OCR session identity.
- Start, supersede, await, restore, complete, fail, cancel, and end OCR tasks.
- Ignore stale task results and telemetry from previous sessions.
- Preserve completed OCR result for valid reuse within the same session.
- Expose current OCR status and sanitized failure reason.
- Correlate request-log updates with the owning request.

### Non-responsibilities

- Capturing screenshots.
- Calling OCR providers.
- Choosing whether a flow should request OCR.
- Rendering overlay UI.

### Invariants

- Only the current session may mutate current OCR result/failure state.
- Timeout while awaiting does not lose ownership of a still-running task.
- Explicit cancellation clears reusable OCR result state.
- Stale task outcomes are observable only as debug diagnostics, not as current user-facing state.

### Required tests

- Start session, complete successfully, reuse result.
- Supersede session and ignore stale success.
- Supersede session and ignore stale failure.
- Await timeout restores running task.
- Explicit cancellation updates status and request-log state.
- Failure reason is sanitized/bounded for overlay use.
- Repeated cancellation/end calls are idempotent.

## Settings View interface contract

### Callers

- Settings store read/write layer.
- Settings UI components.
- Profile/preset editors.
- Runtime sync policy.
- Backend config sync and migration tests.

### Responsibilities

- Normalize raw persisted settings into safe values.
- Preserve explicit-null semantics.
- Provide canonical defaults and source-aware effective values.
- Resolve profile/preset inheritance where requested.
- Support fixture-based drift tests between Rust defaults/seeding and TypeScript normalization.
- Keep overlapping Rust runtime defaults, Rust settings seeding defaults, and TypeScript UI defaults covered by deterministic drift tests.

### Non-responsibilities

- Invoking backend runtime sync.
- Emitting secondary-window events.
- Rendering settings controls.
- Storing secrets.

### Invariants

- Missing and invalid values use documented defaults.
- Explicit null remains explicit when it means disabled or inherited.
- Effective values expose source information sufficient for tests and UI explanations.
- Defaults cannot silently diverge between runtime and UI without a failing drift test.
- Source metadata includes stored/global/profile/preset/default values and may identify policy-sourced values when policy overlays provide effective settings.
- Pipeline runtime defaults that overlap persisted settings must read from the canonical Rust defaults instead of duplicating literals.

### Required tests

- Missing key default for every touched setting.
- Malformed value fallback for every touched setting type.
- Explicit-null preservation for disabled/inherited settings.
- Profile override beats global default when applicable.
- Default profile/global fallback behavior.
- Legacy settings snapshots migrate/normalize without data loss.
- Cross-layer defaults contract for TypeScript defaults versus Rust canonical constants.
- Rust runtime-default drift test for settings that overlap `PipelineConfig::default()`.

## Runtime Sync Policy contract

### Callers

- Settings mutation helpers.
- Query mutations.
- Tauri invoke wrappers.
- Secondary-window refresh paths.

### Responsibilities

- Classify setting changes by runtime effect.
- Run pipeline sync when required.
- Emit settings-change notification when required.
- Deduplicate sync/events for one logical batch.
- Return enough outcome information for tests and callers to observe what happened.

### Non-responsibilities

- Normalizing setting values.
- Deciding UI layout.
- Persisting settings directly, except through existing settings mutation paths.

### Invariants

- A pipeline-affecting logical change triggers exactly one pipeline sync.
- A secondary-window-affecting logical change triggers exactly one settings-change event.
- Non-runtime changes do not trigger unnecessary sync.
- Policy/license normalization and API-key changes remain supported and observable.

### Required tests

- Pipeline-only setting change.
- Secondary-window-only setting change.
- Both pipeline and secondary-window setting change.
- No-runtime setting change.
- Batch deduplication.
- Policy/license mode change payload preservation.
- API-key change behavior preservation.

## Routing Decision contract

### Callers

- Transcription Flow.
- Request-log update logic.
- Existing embeddings and LLM routing implementations.
- Future routing adapters.

### Responsibilities

- Return a strategy-independent decision.
- Distinguish selected preset, default target, no decision, ambiguity, failure, and cancellation.
- Carry bounded/redacted diagnostics for request logs.
- Preserve existing routing fallback behavior.

### Non-responsibilities

- Running STT Execution.
- Performing LLM rewrite.
- Choosing active profile.

### Invariants

- Unknown preset ids are not valid selected-preset outcomes.
- Default target is distinct from failure/no decision.
- Cancellation priority is preserved.
- Diagnostics cannot leak raw secrets.

### Required tests

- Embeddings selected preset.
- Embeddings default target.
- Embeddings below threshold/no decision.
- Embeddings ambiguous margin.
- LLM selected preset.
- LLM default/none target.
- LLM unknown preset ignored.
- Router provider failure produces failure/no-decision outcome according to documented fallback.
- Transcription Flow uses routing decision without strategy-specific tuple knowledge.

## Profile Resolution contract

### Callers

- Recording pipeline profile selection.
- Transcription Flow profile/preset behavior.
- Quick Ask and Quick Replace flow setup.
- Settings UI tests and contract fixtures.

### Responsibilities

- Match foreground program identity to a profile.
- Select default profile fallback.
- Resolve effective preset.
- Resolve Active Window OCR mode for rewrite, Quick Ask, and Quick Replace.
- Treat disabled/invalid profiles safely.

### Non-responsibilities

- Persisting profile settings.
- Running routing strategies.
- Starting OCR tasks.

### Invariants

- Program matching is deterministic and privacy-preserving in logs.
- Flow-specific OCR mode precedence is explicit.
- Disabled or invalid profiles cannot override safe defaults.
- Matching and effective behavior can be tested separately.

### Required tests

- Case-insensitive full path match.
- Windows prefix normalization.
- Basename and no-exe matching.
- First deterministic match behavior.
- No match uses default/global fallback.
- Disabled profile ignored or safely handled according to implementation decision.
- Rewrite/Quick Ask/Quick Replace OCR precedence matrix.

## Local Provider Lifecycle contract

### Callers

- STT Provider Resolution.
- Pipeline local-provider load/unload/status operations.
- Config sync behavior.
- Provider cache operations.

### Responsibilities

- Compute local-provider cache identity.
- Enforce manual/on-transcribe/on-launch load behavior.
- Prevent managed transport for local providers.
- Validate local-provider readiness.
- Provide safe user-facing failure messages.
- Preserve loaded providers across config sync only when compatible.

### Non-responsibilities

- Cloud provider construction.
- STT Execution retry behavior.
- Downloading or installing local models unless explicitly scoped by a later feature.

### Invariants

- Manual mode does not perform surprise heavy loads.
- Cache identity changes when load-affecting configuration changes.
- Local provider traffic is never routed through managed inference.
- Missing model/configuration fails clearly.

### Required tests

- Manual unloaded provider failure.
- Loaded provider reused with matching identity.
- Changed model/config evicts or bypasses stale provider.
- Managed inference enabled still bypasses local provider transport.
- Feature-disabled local provider fails safely.
- Explicit unload removes all relevant cache entries.

## Provider-Family Seam pre-flight contract

### Callers

- Provider-specific STT/LLM/OCR/embedding modules.
- Managed inference code.
- Request log and cost code.
- Future provider adapters.

### Responsibilities

- Prove whether a proposed seam is real before implementation.
- Identify at least two concrete adapters.
- Describe caller leverage and locality gain.
- Preserve provider-specific behavior and redaction.

### Non-responsibilities

- Creating broad registries for hypothetical future providers.
- Removing provider-specific differences that are user-visible or provider-required.

### Invariants

- One adapter means hypothetical seam; two adapters means real seam.
- The deletion test must show complexity would otherwise reappear across callers.
- Provider-family changes must remain deterministic and contract-tested.

### Required tests

- Existing provider behavior characterization before seam extraction.
- At least two adapters satisfy the shared seam.
- Error/cost/metadata behavior remains provider-specific where required.
- Redaction and privacy expectations remain intact.
