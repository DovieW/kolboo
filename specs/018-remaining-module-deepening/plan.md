# Remaining Module Deepening Plan

## Purpose

Implement the remaining architecture-deepening candidates identified after Spec 017 without creating pass-through abstractions.

The guiding test remains: **the Interface is the test surface**. A provider-family seam is only kept when at least two concrete Adapters use it and deleting it would push duplicated caller logic back into orchestration code.

## Phases

### Phase 0 — naming and guardrails

- Record accepted domain terms in `CONTEXT.md`:
  - Telemetry Mapping
  - Cost Reporting
  - Embeddings Module
  - Recording Orchestration
- Keep prior Provider-Family Seam decisions accurate when reopening a deferred seam.

### Phase 1 — characterization first

- Add pure/unit characterization tests before or alongside each extraction.
- Keep tests deterministic: no network, no API keys, no audio devices, no screenshots, no sleeps.

### Phase 2 — Telemetry Mapping

- Add `app/src-tauri/src/telemetry.rs` as the request-log-to-read-model Module.
- Preserve `request_log.rs` ownership of rich storage, redaction, and text/payload stripping.
- First read model: cost telemetry inputs for Stats/Cost Reporting.

### Phase 3 — Cost Reporting

- Add `app/src-tauri/src/cost/reporting.rs` for provider telemetry parsing and event-level cost report assembly.
- Keep provider pricing tables/formulas under their existing provider modules.
- Keep `stats.rs` responsible for persisted event append/flush/prune/UI invalidation.

### Phase 4 — Embeddings Module

- Wire production embeddings routing through the existing `EmbeddingsProvider` Interface and concrete Adapters.
- Centralize provider defaults and query/document input-type hints in `embeddings/mod.rs`.
- Reuse `router_embeddings_cache::router_embedding_cache_key(...)` for hint cache identity.

### Phase 5 — STT provider construction locality

- Preserve STT Provider Resolution ownership in `pipeline/stt_provider_resolver.rs`.
- Move cloud-provider constructor quirks into `pipeline/stt_cloud_adapters.rs`.
- Keep local-whisper and whisper-server special cases explicit in the resolver because they have different lifecycle/readiness rules.

### Phase 6 — Recording Orchestration

- Extract duplicated command-side phase notification watchers into `recording_orchestration.rs`.
- Keep the pipeline state machine as the source of truth for actual transitions.
- Leave broader request finalization/output ownership in existing session modules unless a later deletion test proves another seam.

### Phase 7 — validation

- Run format before tests/checks.
- Rust-only validation starts with `pnpm -C app cargo:fmt` then `pnpm -C app cargo:test`.
- Final gate: `pnpm -C app check:ci` after Cargo environment guard (`RUSTC_WRAPPER`/`CARGO_BUILD_JOBS`).
