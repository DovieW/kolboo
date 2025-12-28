# TODO (Repository Backlog)

This file is a **comprehensive backlog** of improvements that would make Tangerine more robust, easier to maintain, and nicer to use.

Conventions used below:

- **P0 / P1 / P2**: priority (P0 = urgent correctness/safety, P1 = high-value, P2 = nice-to-have)
- **Effort**: S/M/L/XL (rough sizing)
- **Refs**: pointers into the codebase for where to start

---

## 🚨 P0: Correctness, security, and “it should not surprise users”

### Fix API-key inventory mismatches (Danger Zone + UI)

- **Problem**: The Settings UI supports more API keys than the backend “Danger zone” logic counts/deletes.
  - UI keys include: `assemblyai_api_key`, `speechmatics_api_key` (and others).
  - Backend `delete_all_api_keys` and `get_data_storage_summary` currently enumerate only a subset.
- **Fix**:
  - Centralize the authoritative list of key names (shared constant), or ensure all call sites are updated together.
  - Confirm the “Delete all API keys” button truly wipes everything the UI can set.
- **Priority**: P0
- **Effort**: S
- **Refs**:
  - `app/src/components/settings/ApiKeysSettings.tsx` (`API_KEYS`)
  - `app/src-tauri/src/commands/data.rs` (`delete_all_api_keys`, `get_data_storage_summary`)

### Use OS keychain/credential storage for API keys (or clearly disclose plain-text storage)

- **Problem**: Keys appear to be stored in `settings.json` via `@tauri-apps/plugin-store`, which is typically **plain-text on disk**. That’s a privacy/security footgun unless you’re explicit.
- **Fix options**:

  1. Migrate API keys to an OS credential store (recommended) and keep non-secret settings in the store file.
  2. If staying with on-disk storage: add prominent UI + docs warnings and a threat model.
- **Priority**: P0
- **Effort**: L
- **Refs**:
  - `app/src/lib/tauri.ts` (`setApiKey/getApiKey`, store usage)
  - `app/src/components/settings/ApiKeysSettings.tsx`
  - `docs/NO_SERVER.md` (privacy posture / “no server” implications)

### Revisit CSP being disabled

- **Problem**: Tauri config sets `security.csp: null`. That’s convenient for dev, but it expands the blast radius of XSS-class issues.
- **Fix**: Add a strict-ish CSP for production bundles (and document exceptions if needed).
- **Priority**: P0
- **Effort**: M
- **Refs**:
  - `app/src-tauri/tauri.conf.json` (`"csp": null`)

### Update README: remove server-era instructions and claims

- **Problem**: README still references a Python server, WebRTC, `localhost:8765`, and Pipecat, but the repo has migrated to “no server” (implemented).
- **Fix**:
  - Rewrite install/run docs to match the current Tauri-only architecture.
  - Remove/relocate server sections, diagrams, and provider claims that no longer apply.
- **Priority**: P0
- **Effort**: M
- **Refs**:
  - `README.md` (server instructions)
  - `docs/NO_SERVER.md` (source-of-truth architecture)

---

## P1: UX/product polish that unlocks daily usability

### Rename “Connected” status to something truthful for no-server mode

- **Problem**: Legacy UI concepts (“connected”) can confuse users now that there’s no backend server.
- **Fix**: Follow the idea in `TASKS.md` (e.g. “Ready / Recording / Processing”).
- **Priority**: P1
- **Effort**: S
- **Refs**:
  - `TASKS.md` (Connected Dot)
  - UI status code (likely `app/src/App.tsx`, `app/src/components/HistoryFeed.tsx`, overlay)

### Output modes: align UI + backend + naming

- **Problem**: `TASKS.md` mentions output modes (clipboard/keystrokes/auto-paste). The current UI types show: `paste | paste_and_clipboard | clipboard`.
- **Fix**:
  - Decide the product surface:
    - Clipboard-only
    - Paste
    - Paste+keep clipboard
    - (Optional) keystroke typing fallback for apps that block paste
  - Ensure the backend implements all selected modes consistently.
- **Priority**: P1
- **Effort**: M
- **Refs**:
  - `TASKS.md` (Output Mode Options)
  - `app/src/lib/tauri.ts` (`OutputMode`, `updateOutputMode`)
  - Rust typing/output commands (search for `type_text`, clipboard handling)

### Make hotkey conflict error auto-dismiss

- **Priority**: P1
- **Effort**: S
- **Refs**:
  - `TASKS.md`
  - `app/src/components/HotkeyInput.tsx`

### Fix Logs page loading indicator sizing

- **Priority**: P1
- **Effort**: S
- **Refs**:
  - `TASKS.md`
  - `app/src/components/LogsView.tsx`

### Improve Settings layout (tabs vs accordions)

- **Priority**: P1
- **Effort**: M
- **Refs**:
  - `TASKS.md`
  - `app/src/components/settings/*`

---

## P1: Audio/pipeline correctness & quality

### Speechmatics: make language configurable

- **Problem**: Hard-coded language can reduce accuracy and is a known TODO in code.
- **Fix**:
  - Add a setting (global + optionally per-profile) for language.
  - Thread it through the provider config and request payload.
- **Priority**: P1
- **Effort**: M
- **Refs**:
  - `app/src-tauri/src/stt/speechmatics.rs` (TODO)
  - Settings wiring: `app/src-tauri/src/lib.rs` (defaults), `app/src/lib/tauri.ts` (store)

### Reduce “silent audio hallucinations” across providers

- **Status**: partially addressed (quiet-audio gate exists), but ensure UX is great.
- **Improvements**:
  - When audio is rejected as too quiet, show a clear UI reason + suggested fixes (mic selection, gain, noise gate).
  - Consider enabling `quiet_audio_require_speech` by default when VAD is robust.
  - Maintain a small, transparent “hallucination phrase” filter only as a last resort.
- **Priority**: P1
- **Effort**: M
- **Refs**:
  - `TASKS.md` ("Thank you" hallucinations)
  - `app/src-tauri/src/pipeline.rs` (quiet-audio gate)
  - `app/src-tauri/src/vad.rs`

### Make audio preprocessing understandable (and testable)

- **Problem**: Several preprocessing toggles exist; users need “what does this do?” plus safe defaults.
- **Fix**:
  - Add a short in-UI explanation + recommended presets.
  - Add a repeatable “record test clip” flow (already exists) with clearer diagnostics.
- **Priority**: P1
- **Effort**: M
- **Refs**:
  - `app/src-tauri/src/commands/audio.rs` (test recording)
  - `app/src-tauri/src/audio_capture.rs`, `app/src-tauri/src/audio.rs`

---

## P1: Profiles & configuration consistency

### Actually apply per-profile UI/runtime overrides (or remove them)

- **Problem**: The frontend settings model allows per-profile overrides for UI/runtime behaviors (sound, overlay mode, output mode, etc.), but the Rust profile struct does not include these, and comments indicate the backend may ignore them.
- **Fix**:
  - Option A: Implement per-profile routing end-to-end (foreground app → active profile → apply overrides).
  - Option B: Remove/disable profile fields until supported.
- **Priority**: P1
- **Effort**: L
- **Refs**:
  - `app/src/lib/tauri.ts` (`RewriteProgramPromptProfile` includes UI overrides + comments)
  - `app/src-tauri/src/settings.rs` (`RewriteProgramPromptProfile` fields)
  - Pipeline provider selection: `app/src-tauri/src/pipeline.rs`

### Fix settings schema drift between Rust defaults and TS normalization

- **Problem**: Defaults/keys must match across Rust (`ensure_default_settings`) and TS (`getSettings` normalizers). Drift causes odd startup behavior.
- **Fix**:
  - Add a single “settings schema” doc or codegen approach.
  - Add sanity checks (e.g., versioned settings + migrations).
- **Priority**: P1
- **Effort**: L
- **Refs**:
  - `app/src-tauri/src/lib.rs` (`ensure_default_settings`)
  - `app/src/lib/tauri.ts` (`getSettings`)

---

## P2: Performance, reliability, and maintainability

### Improve cancellation/backpressure and user feedback

- Ensure every long operation (recording/transcribe/LLM rewrite) cancels quickly.
- Show “Cancelled” as a first-class status with no scary error formatting.
- **Refs**:
  - `app/src-tauri/src/pipeline.rs`
  - `app/src-tauri/src/commands/recording.rs`
  - `app/src/OverlayApp.tsx`

### Reduce disk/memory pressure for logs and stats

- Add explicit caps + UX controls for:
  - request log retention (already exists)
  - recordings storage size and pruning strategy
  - stats JSONL shard compaction
- **Refs**:
  - `app/src-tauri/src/request_log.rs`
  - `app/src-tauri/src/stats.rs`
  - `app/src-tauri/src/recordings.rs`

### Add more automated tests (unit + a few integration)

- Rust: provider payload builders, cost estimation edge cases, settings migrations.
- TS: settings normalization, hotkey validation.
- **Refs**:
  - `app/src-tauri/src/tests/`
  - `app/src/lib/tauri.ts`

---

## Docs, transparency, and licensing hygiene

### Add a Privacy & Data Handling doc (and link it prominently)

Include:

- what is stored locally (settings, API keys, history, recordings, logs, cost events)
- what is sent to providers (audio, prompts, metadata)
- how to delete everything (Danger Zone)
- how “no server” changes the threat model
- explicitly state whether there is any telemetry/analytics (and if none, say so)
- **Priority**: P1
- **Effort**: M
- **Refs**:
  - `docs/NO_SERVER.md`
  - `app/src-tauri/src/commands/data.rs`

### Add a Troubleshooting / FAQ doc that matches the current architecture

- **Problem**: Common failure modes (mic permissions, audio device selection, hotkey conflicts, provider errors, silent-audio gating) need a single canonical place.
- **Fix**: Add `docs/TROUBLESHOOTING.md` and link it from the README and in-app help.
- **Priority**: P1
- **Effort**: M
- **Refs**:
  - `README.md` (currently has server-era troubleshooting)
  - `app/src/components/settings/SettingsGuideOverlay.tsx`

### Add THIRD_PARTY_NOTICES / dependency licenses

- Useful for transparency and distribution hygiene.
- **Priority**: P2
- **Effort**: M

---

## Tooling & CI/CD

### Expand CI beyond Windows-only release builds

- Add macOS build, Linux build (if supported), and PR workflows.
- Re-enable tests/typecheck/lint in CI when stable.
- Add release hardening: artifact signing/notarization where applicable.
- **Priority**: P1
- **Effort**: M
- **Refs**:
  - `.github/workflows/windows-build.yml` (tests currently disabled, Windows-only)

### Make root package.json less confusing

- Either remove it (if safe) or convert it into a true workspace root with correct scripts.
- **Priority**: P2
- **Effort**: S
- **Refs**:
  - `package.json` (root)
  - `app/pnpm-workspace.yaml`

---

## Nice-to-have features (P2)

- Better onboarding “mic level check” + guided fixes
- Per-app output behavior (e.g., don’t hit Enter in Slack, do hit Enter in chat apps)
- Provider health checks / rate-limit surfacing
- Export/import settings profiles (portable JSON)

---

## Notes

- This repo is licensed under **AGPL-3.0** (`LICENSE`) and the Tauri bundle metadata declares `AGPL-3.0` (`app/src-tauri/tauri.conf.json`). Consider adding a short “License & obligations” section to the README so downstream users are not surprised.
