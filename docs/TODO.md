# TODO (code-grounded backlog)

This backlog is based on reading the current **code and configuration** in this repo (primarily `app/src-tauri/src/**` and `app/src/**`). It intentionally **does not use existing documentation** as a source of truth.

Legend:

- **P0**: likely bug/crash/security/data loss
- **P1**: high-impact improvement (stability, UX, privacy, perf)
- **P2**: nice-to-have / cleanup

---

## P0 — Bugs / correctness

### Fix the LLM command config path (currently a stub)

- **Where:** `app/src-tauri/src/commands/llm.rs`
- **Problem:** `get_current_pipeline_config()` returns `PipelineConfig::default()` (explicitly a placeholder). Commands that rely on it (`update_llm_config`, `update_llm_prompts`, `get_llm_config`) can silently read/overwrite the _wrong_ config.
- **Todo:**
  - Expose a real read-only getter on `SharedPipeline` (e.g., `config_cloned()`), then replace the placeholder.
  - Add a regression test ensuring `update_llm_config` does not reset unrelated pipeline config.

### Make VAD sample rate + frame duration handling consistent

- **Where:** `app/src-tauri/src/vad.rs`, `app/src-tauri/src/settings.rs`
- **Problem:** `VadConfig` contains `sample_rate`, but `VoiceActivityDetector::new()` always sets WebRTC VAD to 16kHz and `frame_size()` assumes 16kHz. Also `frame_duration_ms` isn’t validated (WebRTC VAD only supports 10/20/30ms).
- **Todo:**
  - Either remove `sample_rate` from `VadConfig` (if 16kHz is always the design), or correctly support the full WebRTC VAD sample rate set.
  - Validate/normalize `frame_duration_ms` to 10/20/30.

### Prevent audio-capture stop from potentially hanging

- **Where:** `app/src-tauri/src/audio_capture.rs`
- **Problem:** `AudioCapture::stop()` comments “with timeout” but does an unconditional `join()`; if a driver callback deadlocks, shutdown could hang the app.
- **Todo:**
  - Implement a real timeout strategy (e.g., cooperative shutdown + join with a max wait; log + detach if exceeded).

### Clipboard/paste injection: make clipboard restoration fail-safe

- **Where:** `app/src-tauri/src/commands/text.rs`
- **Problem:** paste modes do best-effort clipboard restore. If clipboard access fails mid-flight, user clipboard may be left overwritten.
- **Todo:**
  - Use a guard pattern (RAII) to attempt restore in all paths.
  - Add a “privacy mode” option: never read prior clipboard (don’t restore), avoiding sensitive clipboard reads.
  - Improve UI error messaging and remediation tips per OS.

---

## P1 — Security & privacy hardening

### Keep CSP restrictive (and prevent accidental regression)

- **Where:** `app/src-tauri/tauri.conf.json`
- **Current state:** CSP is enabled, and a looser `devCsp` is configured for development.
- **Todo:**
  - Add a simple guardrail so release builds can’t accidentally ship with `security.csp: null`.
  - Review whether any directives can be tightened further (e.g. reduce `connect-src` surface) without breaking Tauri/Vite.

### Store API keys in OS secure storage

- **Where:** keys currently live in `settings.json` via store usage (e.g., `app/src-tauri/src/commands/data.rs`, `commands/config.rs`).
- **Problem:** implies plaintext-at-rest in the app data dir.
- **Todo:**
  - Migrate secrets to secure storage (e.g., Tauri Stronghold / OS credential vault).
  - Add migration: read old keys once, write to secure store, delete from settings.
  - Add “export/import settings” that defaults to _excluding_ secrets.

### Tighten request-log redaction guarantees

- **Where:** request log captures `stt_request_json`, `stt_response_json`, `llm_request_json`, `llm_response_json` in `app/src-tauri/src/request_log.rs` and provider implementations.
- **What’s good already:** some providers explicitly omit Authorization in logged request JSON.
- **Todo:**
  - Centralize a `redact_json(Value) -> Value` helper and ensure every provider uses it.
  - Add tests that assert no captured log JSON contains common key patterns (e.g., `Bearer`, `sk-`, etc.).

### Add explicit consent + minimization for window enumeration

- **Where:** `app/src-tauri/src/windows_apps.rs` (+ commands in `commands/windows.rs`)
- **Problem:** window titles can contain sensitive content; API exposes title + executable path.
- **Todo:**
  - Add a permission/consent gate before enabling window title collection.
  - Add a setting to collect only process path (no title) for the per-program profile picker.

### Avoid logging sensitive foreground app paths/titles

- **Where:** `app/src-tauri/src/pipeline.rs` (debug log includes full foreground executable path)
- **Problem:** even if never sent off-device, logs can wind up in bug reports. Full paths can reveal usernames, installed apps, and work context.
- **Todo:**
  - Redact paths in logs by default (e.g., log basename only) or gate behind a “verbose diagnostics” toggle.
  - Ensure window titles are never logged unless explicitly enabled.

### Restrict Tauri capabilities to least privilege

- **Where:** `app/src-tauri/capabilities/default.json`
- **Problem:** `opener:default`, `store:default`, and other broad permissions are enabled for both `main` and `overlay` windows.
- **Todo:**
  - Split capabilities by window: give `overlay` only what it needs (likely window + event listening) and keep `store` writes confined to `main`.
  - Restrict opener to safe schemes/hosts (e.g., `https:` only) and document what can be opened.
  - Add a short “permissions inventory” section in-app (see disclosures section below).

---

## P1 — Performance & responsiveness

### Avoid allocations in real-time audio callbacks

- **Where:** `app/src-tauri/src/audio_capture.rs`
- **Problem:** CPAL callback paths for i16/u16 allocate `Vec<f32>` per callback, and VAD sends freshly allocated `Vec<f32>` chunks via channel.
- **Todo:**
  - Move format conversion and VAD chunking off the realtime callback (ring buffer + worker thread).
  - Avoid per-callback heap allocations; preallocate buffers or use chunk pools.

### Replace `AudioBuffer` front-drain trimming with a ring buffer

- **Where:** `app/src-tauri/src/audio_capture.rs` (`AudioBuffer::append`)
- **Problem:** draining from the front can be O(n) and expensive for long recordings.
- **Todo:**
  - Use a ring buffer or chunked `VecDeque<Vec<f32>>` and drop whole chunks.

### Reduce overlay polling and unify with events

- **Where:** overlay UI (observed polling pattern in `app/src/OverlayApp.tsx`), backend emits multiple pipeline/overlay events.
- **Problem:** state polling + events can cause redundant work and race-y UX.
- **Todo:**
  - Emit a single backend `pipeline-state-changed` event on every transition.
  - Prefer event-driven UI; keep polling as a fallback/backstop.

### Stop polling pipeline state every 500ms in the overlay

- **Where:** `app/src/OverlayApp.tsx` (calls `pipeline_get_state` on a 500ms interval)
- **Problem:** constant polling creates needless backend churn and can race with event-driven updates.
- **Todo:**
  - Poll only when the overlay is visible/active, and back off when idle (e.g., 5–15s) or disable entirely when events are reliable.
  - Consider having the backend push the canonical pipeline state (see event unification item above).

### Unify overlay pipeline state sources (hotkey + events + polling)

- **Where:** `app/src/OverlayApp.tsx`
- **Problem:** `pipelineState` is set via 3 channels:
  - interval polling (`pipeline_get_state`)
  - hotkey events (`recording-start` / `recording-stop`)
  - pipeline events (`pipeline-*`)
- **Todo:**
  - Introduce a reducer/state-machine for UI state so transitions are consistent.
  - Reduce event surface area by consuming one canonical `pipeline-state-changed` event.

### Improve stats aggregation performance

- **Where:** `app/src-tauri/src/commands/stats.rs`
- **Problem:** cost summary endpoints scan all JSONL lines each query.
- **Todo:**
  - Add an incremental index/cache (daily totals, per-provider totals) updated on append.
  - Support a “rebuild index” path if shards are corrupted.

### Reduce fsync/flush overhead for stats writes

- **Where:** `app/src-tauri/src/stats.rs` (`StatsStore::append_cost_event` flushes every event)
- **Todo:**
  - Consider batching appends (buffered writer per day file, periodic flush) while keeping crash-safety reasonable.

---

## P1 — Architecture & maintainability

### Break up oversized files (“god modules”)

- **Where:** `app/src/OverlayApp.tsx`, `app/src-tauri/src/lib.rs`
- **Todo:**
  - Split overlay into smaller components + a dedicated overlay state machine/hook.
  - Split Rust `lib.rs` into focused modules (tray, shortcuts, window mgmt, pipeline session, retention, etc.).

### Centralize settings schema + migrations

- **Where:** frontend normalization/migration logic in `app/src/lib/tauri.ts` and backend default seeding logic.
- **Problem:** duplication increases drift risk.
- **Todo:**
  - Define a single settings schema source (JSON Schema or Rust struct + generated TS types).
  - Add explicit settings versioning + migrations.
  - Add a startup “settings doctor” command: validate + normalize.

### Don’t rely on “remember to emit settings-changed”

- **Where:** `app/src/lib/tauri.ts`, multiple settings UIs
- **Problem:** some settings update APIs emit `settings-changed`, but many do not; callers often manually call `tauriAPI.emitSettingsChanged()` after mutations.
- **Todo:**
  - Create a single settings write helper that always: set → save → emit a structured payload (changed keys, optional revision).
  - Have overlay apply payload immediately for UX-critical fields (accent), then optionally reload from disk for full sync.
  - Add a monotonically increasing `settings_revision` to prevent stale reloads from winning.

### Move settings migrations out of UI components

- **Where:** `app/src/components/settings/PromptSettings.tsx`
- **Problem:** migrations triggered by visiting a screen can leave installs in partially-migrated states.
- **Todo:**
  - Relocate all migrations into centralized settings load/normalize code (TS or Rust) so they run deterministically at startup.
  - Keep UI components migration-free (or at least idempotent and telemetry/logged).

### Make store reads consistent (reload vs cached)

- **Where:** some code reloads the store before reading (e.g., transcription retention), other reads use the cached store directly.
- **Todo:**
  - Introduce a single helper for settings reads that supports “fresh read” semantics.
  - Audit commands that depend on immediately-updated values (retention settings, API key presence).

---

## P1 — UX & product improvements

### Make destructive actions safer and more transparent

- **Where:** `app/src-tauri/src/commands/data.rs` (+ UI uses `get_data_storage_summary`)
- **Todo:**
  - Require typed confirmation for “delete all data”.
  - Emit events after each delete so UI refreshes (history/stats/settings).
  - Add “delete only transcripts but keep recordings” and the reverse.

### Add a unified “Data retention” settings page

- **Where:** retention exists for history (time-based transcription retention), request logs retention, and stats retention.
- **Todo:**
  - Present one coherent UI: what is stored, where, and how long.
  - Show estimated impact (counts/bytes) via `get_data_storage_summary`.

### Improve “retry” UX

- **Where:** retry path exists (`pipeline_retry_transcription` in `commands/recording.rs`).
- **Todo:**
  - Add “Retry last failed request” as a hotkey/menu action.
  - Persist enough context to make retry reliable and visible across restarts.

### Expand output-mode options

- **Where:** `app/src-tauri/src/commands/text.rs`
- **Todo:**
  - Expose “hit enter” option in UI (backend supports it).
  - Add per-program output-mode overrides (e.g., clipboard-only for terminals).

---

## P2 — Observability & debugging

### Make request logs exportable (sanitized)

- **Where:** request logs are in-memory (`app/src-tauri/src/request_log.rs`) and surfaced via commands.
- **Todo:**
  - Add “Export logs” to JSON file and “Copy sanitized log” buttons.
  - Optional redaction mode: strip transcript text while keeping timings and error info.

### Handle mutex poisoning more gracefully

- **Where:** `RequestLogStore` uses `Mutex::lock().unwrap()`.
- **Todo:**
  - Switch to poison-aware locking (or `parking_lot`) so one panic doesn’t cascade into a permanent crash.

---

## P2 — Tooling, CI/CD, and repo hygiene

### Turn CI tests back on and broaden the matrix

- **Where:** `.github/workflows/windows-build.yml` has a disabled test job (`if: ${{ false }}`).
- **Todo:**
  - Re-enable tests for PRs/tags.
  - Add macOS + Linux workflows for build/test parity.

### Add lightweight PR checks

- **Todo:**
  - Run TypeScript typecheck and Rust unit tests.
  - Keep full release bundling on tags only.

---

## Transparency & in-app disclosures (derived from code behavior)

### Add an in-app “Privacy & Data” page

- **Based on code:** the app can record mic audio, send audio/transcripts to third-party STT/LLM providers, and persist local artifacts:
  - `history.json` (history)
  - `recordings/*.wav` (recordings)
  - `stats/cost-events-*.jsonl` (usage/cost ledger)
  - request logs in memory (debugging)
- **Todo:**
  - Provide a plain-language page describing what is stored/sent.
  - Link directly to existing delete/retention controls.

### Add an in-app licensing + third-party services disclaimer

- **Todo:**
  - Show AGPL summary + link to full license.
  - Clearly state that third-party APIs may incur costs and may have their own data retention.

---

## Provider-specific enhancements

### Speechmatics language configurability

- **Where:** `app/src-tauri/src/stt/speechmatics.rs`
- **Problem:** existing inline TODO to make language configurable.
- **Todo:**
  - Add a setting + optional per-program override for Speechmatics language.

### Improve provider error mapping

- **Todo:**
  - Map common HTTP/auth/rate-limit errors into actionable UI messages.
  - Ensure request logs include sanitized error payloads for debugging.
