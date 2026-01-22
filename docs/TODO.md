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

## P1 — UX & product improvements

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
  - Add per-program output-mode overrides (e.g., clipboard-only for terminals).

---

## P2 — Observability & debugging

---

## P2 — Tooling, CI/CD, and repo hygiene

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

