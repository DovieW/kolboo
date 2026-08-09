# Architecture guardrails

This note preserves the good parts of the architecture-deepening work without turning every future task into another refactor pass.

## Default stance

Default to **feature delivery**.

The current Module ownership lines are already much healthier than they were before the architecture-deepening initiative. New work should usually extend those Modules instead of reopening architecture by default.

## When to deepen a Module

Deepen a Module only when the change increases **leverage** for callers and **locality** for maintainers.

A refactor is usually worth doing when **all** of these are true:

- the current Interface is forcing callers to repeat real behavior or invariants
- the **deletion test** passes: deleting the Module would re-spread complexity across multiple callers/tests
- the new Interface is simpler than the behavior it hides
- tests can exercise the important behavior through the Interface rather than by reaching past it

## When not to deepen

Do **not** add a new Module or seam just because code feels large or because two implementations rhyme.

Usually avoid refactoring when the proposed Module would be:

- a pass-through wrapper
- a generic manager/coordinator that mostly forwards work elsewhere
- a provider-family seam with only one real Adapter
- a split that makes maintainers open more files without gaining clearer ownership

## Seam rules that should stay true

### Feature-first, seam-aware changes

When an area already has a good deep Module, prefer adding behavior there instead of creating a nearby helper with overlapping ownership.

Examples already in good shape:

- settings normalization Modules under `app/src/lib/tauri/settingsNormalizers/**`
- `settingsViews.ts` / backend `settings_view.rs` for source-aware settings reads
- provider-local realtime STT Modules under `app/src-tauri/src/stt/**/realtime.rs`
- `history/orchestration.ts`, `logs/orchestration.ts`, and `dataRetention.ts` / `dataBackupCloudSync.ts` for frontend non-rendering orchestration
- `cost/reporting.rs`, `telemetry.rs`, and `history_request_lifecycle.rs` for focused backend ownership

### Provider-family seams require proof

A provider-family seam is only real when at least two concrete Adapters need the same Interface **and** the deletion test passes.

Until then, keep provider-specific protocol/state behavior in the concrete Adapter.

Current decisions:

- Realtime WebSocket proxy, TLS, and `no_proxy` behavior is a proven shared transport seam in `app/src-tauri/src/stt/websocket_transport.rs`.
- Provider-independent WebSocket/session lifecycle belongs in `app/src-tauri/src/stt/streaming.rs`.
- Realtime transcript protocol state machines remain provider-local under `app/src-tauri/src/stt/*/realtime.rs`; a broad cross-provider parser interface was considered and rejected because provider event, commit, and shutdown semantics dominate the similarity.
- `LlmProvider::complete_json_schema(...)` remains the caller-facing structured-output seam. Do not wrap it in another family abstraction without new duplicated behavior in at least two adapters.
- Shared LLM client configuration and any OCR provider interface remain deferred until two adapters demonstrate a concrete shared concern.

## How to handle out-of-scope architecture pain

If feature work exposes a real refactor opportunity but it is too large for the current task:

1. keep the current task scoped
2. add a specific note to `docs/Refactors/*.md`
3. include files, pain points, and what would make the refactor worth revisiting

This keeps the backlog grounded in real code friction instead of abstract architecture taste.

## Current low-priority follow-ups

These are worth revisiting only if future feature work reopens them:

- the split between `pipeline/ocr_session.rs` and `pipeline/ocr_session_state.rs`
- the multi-Module recording command lifecycle packaging across `recording_lifecycle.rs`, `recording_request_initialization.rs`, `recording_orchestration.rs`, and `recording_completion.rs`

Those are **not** blockers right now. The current architecture is good enough to spend on features.
