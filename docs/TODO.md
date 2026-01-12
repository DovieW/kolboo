# TODO (code-grounded backlog)

This backlog is based on reading the current **code and configuration** in this repo (primarily `app/src-tauri/src/**` and `app/src/**`). It intentionally **does not use existing documentation** as a source of truth.

Legend:

- **P0**: likely bug/crash/security/data loss
- **P1**: high-impact improvement (stability, UX, privacy, perf)
- **P2**: nice-to-have / cleanup

## P1 — Security & privacy hardening

### Consider tightening CSP directives further

- **Where:** `app/src-tauri/tauri.conf.json`, release guardrail in `app/src-tauri/build.rs`
- **Current state:** CSP is enabled; `build.rs` fails release builds if `security.csp` is missing/empty.
- **Todo:**
  - Review whether any directives can be tightened further (e.g. reduce `connect-src` surface) without breaking Tauri/Vite.

---

## P1 — Performance & responsiveness

### Avoid per-callback heap allocations in audio callbacks (done)

- **Where:** `app/src-tauri/src/audio_capture.rs`
- **Previous problem:** CPAL callback paths for i16/u16 allocated `Vec<f32>` per callback, and VAD handoff allocated `Vec<f32>` chunks.
- **Current state:** callbacks reuse preallocated conversion scratch buffers, and VAD handoff uses a small `Vec<f32>` pool so steady-state capture avoids heap allocation.

### Move audio processing off the realtime callback

- **Where:** `app/src-tauri/src/audio_capture.rs`
- **Problem:** callback still does non-trivial work (format conversion/downmix) and takes mutex locks (`buffer`, `pre_roll`).
- **Todo:**
  - Introduce a ring buffer + worker thread so the CPAL callback only enqueues samples.
  - Do buffer append / pre-roll maintenance / VAD feeding on the worker thread.

### Replace `AudioBuffer` front-drain trimming with a ring buffer (done)

- **Where:** `app/src-tauri/src/audio_capture.rs` (`AudioBuffer::append`)
- **Previous problem:** draining from the front could be O(n) and expensive for long recordings.
- **Current state:** `AudioBuffer` is now a fixed-capacity ring buffer that overwrites the oldest samples in O(1).

### Unify overlay pipeline state sources (hotkey + events + polling)

- **Where:** `app/src/OverlayApp.tsx`
- **Problem:** `pipelineState` is set via 3 channels:
  - interval polling (`pipeline_get_state`)
  - hotkey events (`recording-start` / `recording-stop`)
  - pipeline events (`pipeline-*`)
- **Todo:**
  - Introduce a reducer/state-machine for UI state so transitions are consistent.

### Improve stats aggregation performance

- **Where:** `app/src-tauri/src/commands/stats.rs`
- **Problem:** cost summary endpoints scan all JSONL lines each query.
- **Todo:**
  - Add an incremental index/cache (daily totals, per-provider totals) updated on append.
  - Support a “rebuild index” path if shards are corrupted.

### Reduce fsync/flush overhead for stats writes (done)

- **Where:** `app/src-tauri/src/stats.rs`
- **Previous problem:** `StatsStore::append_cost_event` opened a file + created a writer + flushed on every event.
- **Current state:** the stats shard file is kept open with a buffered writer and is flushed once per request (after all cost events are appended), reducing overhead while keeping the UI reads fresh.

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
