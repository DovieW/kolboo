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

### Move audio processing off the realtime callback

- **Where:** `app/src-tauri/src/audio_capture.rs`
- **Problem:** callback still does non-trivial work (format conversion/downmix) and takes mutex locks (`buffer`, `pre_roll`).
- **Todo:**
  - Introduce a ring buffer + worker thread so the CPAL callback only enqueues samples.
  - Do buffer append / pre-roll maintenance / VAD feeding on the worker thread.

### Unify overlay pipeline state sources (hotkey + events + polling)

- **Where:** `app/src/OverlayApp.tsx`
- **Problem:** `pipelineState` is set via 3 channels:
  - interval polling (`pipeline_get_state`)
  - hotkey events (`recording-start` / `recording-stop`)
  - pipeline events (`pipeline-*`)
- **Current state:** overlay now routes pipeline state + animation state through a small reducer, and treats polling as a backstop with a short suppression window after event/hotkey/UI updates (reduces flicker/races).
- **Todo (optional follow-up):**
  - Extract the reducer into a dedicated hook (`useOverlayUiReducer`) and add a transition table comment/tests for tricky cases.

### Improve stats aggregation performance

- **Where:** `app/src-tauri/src/commands/stats.rs`
- **Previous problem:** cost summary endpoints scanned all JSONL lines on every query.
- **Current state:** stats queries are now cached in-memory and invalidated whenever a new cost event is appended.
- **Todo (optional follow-up):**
  - Add an on-disk incremental index (for instant stats even after restart) and a “rebuild index” path if shards are corrupted.

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

### Improve provider error mapping

- **Todo:**
  - Map common HTTP/auth/rate-limit errors into actionable UI messages.
  - Ensure request logs include sanitized error payloads for debugging.
