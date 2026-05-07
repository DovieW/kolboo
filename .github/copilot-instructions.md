# Kolboo Copilot instructions

- Keep instructions in sync:

  - If your change is significant enough (or it overlaps with guidance here or in `.github/instructions/**`), update the relevant instruction file(s) in the same PR.
  - Typical triggers: renamed/moved files, changed recommended commands, updated settings behavior, added/renamed Tauri commands/events, changed testing/CI expectations.

- Common commands:

  - `pnpm -C app dev` (Tauri dev)
  - `pnpm -C app build` (Tauri build)
  - `pnpm -C app check` (aggregates Biome/tsc/knip/vitest + Rust helpers)
  - `pnpm -C app check:ci` (CI gate; preferred before merging)

- Local Rust build cache (important for agent-run commands):

  - Before running any command that invokes Cargo/Rust locally (`pnpm -C app dev`, `build`, `cargo:*`, `test:all`, `check`, `check:ci`), set `RUSTC_WRAPPER=sccache` in the current shell when `sccache` is available.
  - PowerShell guard (preferred): if `sccache` exists, set wrapper; otherwise clear `RUSTC_WRAPPER` so Cargo falls back to plain `rustc`.
  - To avoid saturating the whole machine, also set a conservative `CARGO_BUILD_JOBS` for local runs (recommended: about half logical cores, capped around 8).
  - This improves incremental performance and reduces repeated recompilation cost across runs.

- Repo layout:

  - UI (React/Vite/TS) lives in `app/src/**`.
  - Tauri backend (Rust) lives in `app/src-tauri/src/**`.
  - The core recording pipeline/state machine is `app/src-tauri/src/pipeline.rs`; app bootstrap + command registration is `app/src-tauri/src/lib.rs`.

- UI↔backend contract:

  - UI calls Rust commands via `@tauri-apps/api/core` (`invoke`) and wrappers in `app/src/lib/tauri.ts`.
    - The wrappers are split into modules under `app/src/lib/tauri/**`:
      - `tauri/commands.ts` (invoke wrappers)
      - `tauri/settings.ts` (settings read/write + normalization/migrations)
      - `tauri/types.ts` (shared TS types)
  - UI state is mostly TanStack Query hooks in `app/src/lib/queries.ts`.
  - When changing any setting that affects runtime behavior, persist to the Tauri store **and** call `configAPI.syncPipelineConfig()` so the Rust `PipelineConfig` updates immediately.
  - After changing settings that overlays depend on (accent, widget position, overlay mode, etc.), emit `settings-changed` so secondary windows refresh cached settings (see `tauriAPI.update*` helpers).
  - Secondary windows should respond to `settings-changed` by reloading/invalidation only; do **not** call `sync_pipeline_config` from overlay listeners because the owning settings mutation path already applies the Runtime Sync Policy.

- Settings conventions (very important):

  - `settings.json` is the canonical persisted store for non-secret settings/cache (`@tauri-apps/plugin-store`).
  - API keys and session/refresh secrets belong in OS secure storage via `app/src-tauri/src/secrets.rs`; legacy `settings.json` API-key fallback exists only for migration/backward compatibility.
  - `null` often means “explicitly disabled”; missing/invalid values should fall back to defaults.
  - If you add/rename a setting, update BOTH:
    - Rust default seeding/migrations (see `ensure_default_settings(...)` in `app/src-tauri/src/lib.rs`)
    - TS normalization/migrations in the settings layer (`app/src/lib/tauri/settings.ts`) and any UI that reads it.

- Profiles/presets/router:

  - Per-program behavior is configured via `rewrite_program_prompt_profiles` (UI types in `app/src/lib/tauri/types.ts`, backend mapping in `app/src-tauri/src/pipeline.rs`).
  - Profiles can contain presets + an intent router (embeddings or LLM) that selects a preset; keep backward-compatibility for older `settings.json` shapes.

- Overlay windows:

  - Overlay is a separate Vite entry (`overlay.html`) with entrypoint `app/src/overlay-main.tsx` and UI in `app/src/OverlayApp.tsx`.
  - Overlay polls `pipeline_get_state` and listens to backend events (e.g., `overlay-hide-requested`, `overlay-audio-level`). Keep event names/payloads in sync with Rust emitters.
  - Overlay uses `PipelineState` strings; `arming` is UI-only (Rust won’t return it).

- Pipeline safety patterns:

  - The backend pipeline is a state machine; prefer explicit guard methods/transitions over ad-hoc flags.
  - Cancellation is part of the UX (escape-to-cancel is registered only while active). Avoid re-entrant shortcut registration; follow the existing lock/async pattern in `app/src-tauri/src/shortcuts/mod.rs`.
  - Hotkey startup/runtime registration decisions live in `app/src-tauri/src/shortcuts/lifecycle.rs`; Windows modifier-only hook mechanics stay in `app/src-tauri/src/windows_modifier_hotkeys.rs`, and shortcut dispatch stays in `shortcuts/mod.rs`.
  - Shared audio format normalization (downmixing, PCM conversion, streaming chunk sizing, latency-friendly streaming resampling, and VAD-quality 16 kHz resampling) lives in `app/src-tauri/src/audio_normalization.rs`; STT streaming transport/session lifecycle helpers (WS connect/read/send/close and `StreamingSttSession`) live in `app/src-tauri/src/stt/streaming.rs`; provider protocol state machines stay in provider adapters.
  - History request-row transitions live in `app/src-tauri/src/history_request_lifecycle.rs`, while `app/src-tauri/src/history.rs` continues to own persistence/querying. Prefer `RequestHistoryUpdate` + the lifecycle helper over hand-rolled `add_request_entry` / `set_request_*` / `complete_request_*` / `delete` sequences in app-facing flows.
  - Shared OCR usage policy for stop-time auto-start and Quick Ask / Quick Replace OCR consumption lives in `app/src-tauri/src/sessions/ocr_usage.rs`; keep OCR session/task ownership in `app/src-tauri/src/pipeline/ocr_session_state.rs` and `ocr_session.rs`.
  - Telemetry Mapping from rich request logs into narrow read models lives in `app/src-tauri/src/telemetry.rs`; keep request-log storage/redaction/export stripping in `app/src-tauri/src/request_log.rs`.
  - Use `RequestLogStore::start_request_with(...)` when a command starts a request and immediately seeds profile/model/kind metadata; this keeps request-log lifecycle initialization atomic instead of `start_request(...)` plus a separate `with_current(...)` pass.
  - Command-facing recording completion helpers (saved WAV persistence plus final transcript/cancel/error event shapes) live in `app/src-tauri/src/recording_completion.rs`; request-log success metadata, cost completion, OCR cleanup, and History preset/LLM metadata mirroring remain in `app/src-tauri/src/sessions/recording_finalization.rs`; keep platform output in `normal_dictation_output.rs` and Quick Ask/Quick Replace execution in `quick_action_execution.rs`.
  - Cost Reporting event assembly (provider response parsing, token mapping, duration fallback, and event-level estimate selection) lives in `app/src-tauri/src/cost/reporting.rs`; provider pricing tables/formulas stay in provider-specific `app/src-tauri/src/cost/**` modules, and `stats.rs` owns persistence/aggregation/retention/UI invalidation.
  - Backend feature-shaped settings reads live in `app/src-tauri/src/settings_view.rs`; use them for output settings, Quick Ask config, retention, and free-tier reads instead of repeating raw settings-store keys in each caller.
  - Embeddings routing should call through the `EmbeddingsProvider` interface in `app/src-tauri/src/embeddings/mod.rs`; do not bypass it with provider-specific routing HTTP calls unless a new two-adapter proof updates the seam evidence.
  - Cloud STT provider constructor quirks live in `app/src-tauri/src/pipeline/stt_cloud_adapters.rs`; STT Provider Resolution remains in `pipeline/stt_provider_resolver.rs`, and local-whisper/whisper-server lifecycle special cases should stay explicit.
  - Batch STT request orchestration (managed-auth refresh retry, `stt_complete` bookkeeping, and shared failure handling for normal batch, streaming fallback, and retry transcription) lives in `app/src-tauri/src/pipeline.rs`; keep provider selection in `pipeline/stt_provider_resolver.rs` and execution/timeout/retry transport in `pipeline/stt_flow.rs`.
  - Command-facing recording phase notification watchers live in `app/src-tauri/src/recording_orchestration.rs`; the pipeline state machine remains the source of truth for actual transitions.
  - Prompt formatting for rewrite/Quick Ask/Quick Replace lives in `app/src-tauri/src/prompt_builders.rs`; keep clipboard transport and context capping in `app/src-tauri/src/clipboard_context.rs`.
  - Quick Ask / Quick Replace context-source collection (selection probe, clipboard context, OCR fetch) lives in `app/src-tauri/src/sessions/context_collection.rs`; provider execution and Quick Action request-log completion live in `app/src-tauri/src/sessions/quick_action_execution.rs`.
  - Normal dictation final output and non-empty success finalization live in `app/src-tauri/src/sessions/normal_dictation_output.rs`; `lib.rs::stop_recording(...)` should remain orchestration rather than owning platform paste/type branches.

- Spec Kit/git workflow:

  - Do not auto-run branch-changing Spec Kit hooks/scripts unless the user explicitly accepts. The active `before_specify` hook is optional because `speckit.git.feature` creates/switches branches internally.

- Formatting/tooling:

  - TypeScript is strict (see `app/tsconfig.json`). Keep types accurate; prefer narrow unions for state/event strings.
  - Biome formats with **tabs** and double quotes (see `app/biome.json`). Avoid drive-by reformatting.

- Security/telemetry:
  - API keys are written through `tauriAPI.setApiKey(...)` to OS secure storage, with legacy `settings.json` fallback only when reading/migrating old installs. Never log secrets; redact request logs where needed.

<!-- SPECKIT START -->
For the active Spec Kit architecture-deepening initiative, read
`specs/017-architecture-deepening-plan/plan.md` for technologies, project
structure, validation commands, sequencing, and quality gates.

Implemented slice conventions from this initiative:

- OCR session ownership is centralized in `app/src-tauri/src/pipeline/ocr_session_state.rs`.
- Settings runtime side effects and settings-related query invalidation intent are classified by `app/src/lib/tauri/settingsSync.ts`.
- Simple local settings mutations in `app/src/lib/queries.ts` should prefer the shared `useSettingsInvalidatingMutation(...)` helper; optimistic/runtime-sync-heavy mutations can stay bespoke.
- Overlay settings-change refresh behavior lives in `app/src/lib/overlay/overlaySettings.ts` and intentionally does not perform runtime pipeline sync.
- Routing strategy outputs flow through the strategy-independent `RoutingDecision` type in `app/src-tauri/src/pipeline/routing.rs`.
- Profile behavior is split between `app/src-tauri/src/pipeline/profile_matcher.rs` and `app/src-tauri/src/pipeline/profile_resolution.rs`.
- Local provider cache/readiness/bypass decisions live in `app/src-tauri/src/pipeline/local_provider_lifecycle.rs`.
- Prompt formatting is centralized in `app/src-tauri/src/prompt_builders.rs`.
- Telemetry Mapping lives in `app/src-tauri/src/telemetry.rs`.
- Cost Reporting event assembly lives in `app/src-tauri/src/cost/reporting.rs` while stats persistence remains in `app/src-tauri/src/stats.rs`.
- Embeddings routing uses the `EmbeddingsProvider` interface and shared cache-key helper instead of direct provider HTTP calls in routing.
- Cloud STT provider construction adapters live in `app/src-tauri/src/pipeline/stt_cloud_adapters.rs`.
- Recording phase notification watchers live in `app/src-tauri/src/recording_orchestration.rs`.
- History request-row lifecycle orchestration lives in `app/src-tauri/src/history_request_lifecycle.rs` while `history.rs` stays focused on persistence and querying.
- Command-facing recording completion helpers live in `app/src-tauri/src/recording_completion.rs` while request-log/cost/OCR completion remains in `app/src-tauri/src/sessions/recording_finalization.rs`.
- Backend feature-shaped settings reads live in `app/src-tauri/src/settings_view.rs`.
- Hotkey lifecycle registration decisions live in `app/src-tauri/src/shortcuts/lifecycle.rs`.
- Terminal recording finalization lives in `app/src-tauri/src/sessions/recording_finalization.rs`.
- STT streaming transport/session lifecycle helpers live in `app/src-tauri/src/stt/streaming.rs`, while pure audio normalization lives in `app/src-tauri/src/audio_normalization.rs`.
- Quick Action context-source collection is centralized in `app/src-tauri/src/sessions/context_collection.rs`.
- Normal dictation final output/non-empty success finalization is centralized in `app/src-tauri/src/sessions/normal_dictation_output.rs`.
- Do not add provider-family seams unless `specs/017-architecture-deepening-plan/validation/provider-family-decisions.md` records a two-adapter proof and deletion-test pass; follow-up seam evidence for the remaining module-deepening slice lives under `specs/018-remaining-module-deepening/validation/`.
<!-- SPECKIT END -->
