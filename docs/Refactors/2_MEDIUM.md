# Medium-priority refactors

<!-- Add medium-priority refactor ideas here. Keep each item specific and code-grounded. -->

## Centralize setting default values (DRY violation)

**Status:** Mostly addressed by architecture-deepening work (2026-05-05)

Setting defaults are currently defined in multiple places, making it easy for them to drift out of sync:

1. **Rust struct defaults** — `pipeline/config.rs` (`PipelineConfig::default()`, `OcrConfig` fields)
2. **Settings store seeding** — `settings/defaults.rs` (`set_default(...)` calls)
3. **Bootstrap loading** — `bootstrap/mod.rs` (`get_setting_from_store` fallback values)
4. **TS normalization** — `settings.ts` (normalize functions return a fallback)
5. **UI components** — `OcrProviderSettings.tsx` (uses `?? "default_value"` inline)

Example: `ocr_auto_capture_timing` default is defined as `"on_start"` in all 5 places.

Ideas:

- Create a `settings_defaults.rs` module that exports typed constants for each setting default.
- Have `PipelineConfig::default()`, `ensure_default_settings()`, and `get_setting_from_store` all reference these constants.
- For TypeScript, generate a `defaults.generated.ts` from the Rust constants (similar to how we generate types from schemas), or define them once in `types.ts` and import everywhere.
- UI components should reference the normalize function's output rather than inline `?? "..."` fallbacks.

Progress (2026-05-05):

- Added `app/src-tauri/src/settings/default_definitions.rs` as the startup seeding definition Module for persisted setting defaults and explicit-null seed rules.
- `settings/defaults.rs::ensure_default_settings(...)` now iterates those definitions and keeps only store-state-dependent migrations inline (Default rewrite profile insertion and derived hotkey shortcut cards).
- Follow-up remains: reduce remaining bootstrap/read-time fallback literals in `bootstrap/mod.rs` and TS normalization once a safe generated/shared default contract is available.

## Broaden Settings View adoption for inherited/preset callers

**Status:** Partially addressed by architecture-deepening work (2026-05-04)

`app/src/lib/tauri/settingsViews.ts` is now used in production for flat persisted settings via `settingValueView(...)`, so the seam is no longer test-only.

Remaining gap:

- `inheritedSettingView(...)`
- `presetSettingView(...)`

still appear to be used mainly by tests rather than by production callers that resolve effective profile/preset settings.

This is **not** a blocker for the recent review fix, because the main issue was that the Settings View seam had no production leverage at all. That part is now resolved.

Future deepening could route effective profile/preset setting reads through these helpers so the UI and settings layer share one source-aware inheritance path for:

- global → profile fallback
- preset → profile → global fallback
- explicit-null/inherit semantics
- future user-facing "where did this value come from?" explanations

Progress (2026-05-05):

- Added `app/src/components/settings/prompt/effectivePromptSettings.ts` to move prompt/profile fallback calculation out of `usePromptSettingsProfileState.ts`.
- The prompt settings hook now uses the shared `settingsViews.ts::isInheritedSettingValue(...)` rule for profile inheritance checks.
- Follow-up remains: route preset-specific editor state through `presetSettingView(...)` when preset editing gets touched again.

## Reduce maintenance cost of schema registry

`app/src-tauri/xtask/src/schema_registry.rs` is currently a hand-maintained list of all JSON Schemas we export.

It works fine, but adding/removing a schema means touching a big list, which is easy to forget and can drift over time.

Ideas:

- Use a macro to declare the registry in a more compact "data-only" way.
- Generate the registry from a single source of truth (a small TOML/JSON manifest, or a Rust module that is codegen'd).
- If we ever add many more schemas, consider splitting the registry by domain (settings, commands, events, etc.) and merging them.

## Consolidate context capture + prompt building

**Status:** Implemented by Quick Action/output deepening work (2026-05-05); follow-up ideas below remain optional.

Context capture for Quick Ask / Quick Replace is currently spread across multiple places:

- Highlighted-selection capture (key injection + clipboard sentinel) lives in `app/src-tauri/src/text/selection_probe.rs` and is orchestrated via `app/src-tauri/src/sessions/selection_probe.rs`.
- Clipboard "extra context" reading lives in `app/src-tauri/src/clipboard_context.rs`.
- Prompt formatting now lives in `app/src-tauri/src/prompt_builders.rs` so clipboard transport/context capping and LLM message assembly stay separate.
- Quick Ask / Quick Replace context-source orchestration now lives in `app/src-tauri/src/sessions/context_collection.rs`; `quick_action_execution.rs` owns provider execution, request-log updates, stats, and Quick Action completion.
- Normal dictation final output execution and non-empty success finalization now live in `app/src-tauri/src/sessions/normal_dictation_output.rs`, including request-log completion, cost stats, OCR cleanup, history updates, and retention after output warnings are recorded.

Ideas:

- If empty-transcript or error/cancel finalization becomes a real pain point, consider extracting that separately with characterization tests; the normal non-empty success path is already delegated.
- Consider wiring up `ContextGrabMethod::ClipboardOnly` end-to-end (it exists in Rust but isn't currently selectable via the `context_grab_method` string mapping in `lib.rs` / settings docs).
- Keep `selection_probe.rs` AppState/epoch mechanics as an adapter detail unless a later deletion test proves a probe result store would add real Depth rather than ceremony.

## Improve Win+C (Copilot) hotkey reliability when Kolboo is focused

**Problem:** When Kolboo's main window is focused and the user presses Win+C (or physical Copilot key via Win+Shift+F23), Windows intercepts the key combination at the OS level before it reaches either the WH_KEYBOARD_LL hook or JavaScript event handlers. This causes the OS Copilot to launch instead of triggering Kolboo's action.

**Context:**
- AltRight was fixed by adding a JavaScript handler in `useModifierKeyForwarder.ts` that captures the key before Chromium's menu accelerator handling.
- Win+C cannot be captured the same way because Windows itself intercepts Win-key combinations at the OS level.
- The hook DOES receive keyup events for these keys, just not keydown.

**Current state:**
- AltRight hotkey works fine even when Kolboo is focused (via JS forwarder).
- Win+C/Copilot key bypasses to OS Copilot when Kolboo is focused.
- Works correctly when other apps are focused.

**Potential solutions:**
1. **RegisterHotKey approach:** Instead of using the keyboard hook for Copilot, register Win+C as a global hotkey via `tauri-plugin-global-shortcut`. This would work even when Kolboo is focused since RegisterHotKey operates at the OS level.
2. **Keyboard filter driver:** More invasive but would work — requires elevated privileges.
3. **Document as a limitation:** If the above are too complex, document that Copilot key doesn't work when Kolboo is focused and suggest alternatives (use a different hotkey, or don't focus Kolboo while using Copilot key).

**Files involved:**
- `app/src-tauri/src/windows_modifier_hotkeys.rs` — WH_KEYBOARD_LL hook
- `app/src-tauri/src/commands/settings.rs` — `is_windows_hook_handled_hotkey()` determines routing
- `app/src/hooks/useModifierKeyForwarder.ts` — JavaScript key forwarder (already handles AltRight)

## Centralize STT language normalization + mapping

**Status:** Completed (2026-02-03)

Notes:
- Shared helpers now live in `app/src-tauri/src/stt/language.rs`.
- Pipeline uses `PipelineInner::resolve_effective_stt_settings(...)` to avoid duplicated override logic.

## Centralize WebSocket connectivity (proxy + logging + timeouts)

We now have (at least) two STT providers that use WebSockets:

- `SpeechmaticsSttProvider` (realtime WS)
- `ElevenLabsSttProvider` (Scribe v2 realtime WS)

Each implementation currently handles:

- URL construction
- handshake headers
- timeouts
- error mapping
- request logging

And **neither** reliably honors our existing HTTP proxy settings path (we inject a configured `reqwest::Client` for HTTP STT providers, but WS uses direct TCP/WebSocket).

Ideas:

- Add a small shared `ws` helper module (e.g. `app/src-tauri/src/network/ws.rs`) for:
	- consistent timeouts
	- consistent error conversion to `SttError`
	- optional proxy support (where feasible)
	- shared request/response logging conventions
- Standardize WS URL building (query params) + minimal percent-encoding rules.

Progress (2026-02-05):

- Added `app/src-tauri/src/stt/streaming.rs` with shared helpers for:
	- WS connect + timeout/error mapping (`connect_ws_split_with_timeout`)
	- timed receive (`ws_next_with_timeout`)
	- PCM s16le chunk sizing (`chunk_size_bytes_for_pcm_s16le`)
- `ElevenLabsSttProvider` now uses these helpers for its realtime WS path.
- `SpeechmaticsSttProvider` now reuses the shared chunk sizing helper.

Remaining gaps:

- Proxy settings still aren’t consistently applied to WS connections.
- We still have provider-specific request/response log JSON shapes; could standardize a small common subset (endpoint, transport, model_id, chunks_sent, etc.).

## Make CLI output reliably machine-readable

**Problem:** CLI commands currently print a single JSON response, but runtime logs (including JSON-formatted tracing logs) may be written to the same stream. This makes it annoying to consume CLI output programmatically (you have to "find the last JSON object" instead of just parsing stdout).

**Why it matters:** The CLI has become our best tool for latency diagnosis and benchmarking, and downstream tooling (PowerShell scripts, CI smoke checks, etc.) should be able to parse output deterministically.

Ideas:

- In CLI mode, route tracing/log output to **stderr** and reserve **stdout** for the final CLI response JSON.
- Add a `--quiet` flag (or `KOLBOO_CLI_QUIET=1`) that suppresses non-essential logs.
- Consider a `--jsonl` mode for streaming per-run benchmark results without mixing with logs.

**Related pain point:** Some CLI benchmark output includes a `request_log` summary field, but it can be `null` if `RequestLogStore` isn't managed/available in the minimal CLI app setup. Either wire it up consistently for CLI runs or remove the field to avoid confusion.

## Deduplicate STT transcription flow helpers

**Status:** Phase 2 completed (2026-05-01)

There used to be two very similar implementations of `run_stt_transcription(...)`:

- `app/src-tauri/src/pipeline/stt_flow.rs`
- `app/src-tauri/src/pipeline/transcription_flow.rs`

This duplication made it easy for behavior/telemetry (retry handling, timeout semantics, new diagnostics fields) to drift.

Current state:

- `app/src-tauri/src/pipeline/stt_flow.rs` is the canonical STT execution module.
- The unused duplicate helper in `transcription_flow.rs` has been removed.
- Characterization tests cover retry telemetry, optional timeout behavior, non-retryable errors, and cancellation priority.
- `app/src-tauri/src/pipeline/stt_provider_resolver.rs` centralizes transcription-time STT Provider Resolution: profile/preset/global settings, per-run CLI overrides, request-log provider/model metadata, provider cache creation, managed inference routing, Local Whisper/Whisper Server special cases, and global-provider fallback.
- `pipeline.rs` now asks for a resolved STT provider instead of repeating fallback/logging/cache setup in the last-audio test endpoint, main stop/transcribe path, and retry/CLI transcription path.

Remaining ideas:

- If `pipeline.rs` still has too much repeated batch STT → routing → rewrite choreography, consider a follow-up wrapper in `transcription_flow.rs` that composes `stt_flow::run_stt_transcription(...)` with `complete_transcription_flow(...)`.
- Keep streaming finalization and managed-auth retry behavior explicit unless characterization tests make it safe to consolidate them.

## Consolidate duplicated Settings shell components

**Status:** Partially addressed by architecture-deepening work (2026-05-05)

`app/src/App.tsx` currently contains both `_SettingsView` and `SettingsViewWithGuideLauncher`, and they each reimplement nearly the same:

- profile picker state
- modal wiring
- settings tab list
- per-tab panel rendering

This duplication is easy to miss because one of the components is effectively legacy-ish, but every time we add/remove/relabel a settings tab we have to update both copies. The new standalone Account page work touched both just to remove the old Account tab.

Ideas:

- Extract a single shared `SettingsShell` component that owns the tab list and panel rendering.
- Pass optional header actions (like the setup-guide launcher) via props instead of duplicating the whole screen.
- Keep the profile-picker state in one place so tab/view changes do not need mirrored updates.

Progress (2026-05-05):

- Added `app/src/components/settings/SettingsShell.tsx`, and the active Settings route now renders through this shared shell with an optional setup-guide launcher.
- Follow-up remains: delete the legacy `_SettingsView` / legacy wrapper code from `App.tsx` once the shell has baked and no standalone entrypoint relies on the old definitions.

## Deduplicate audio conversion utilities across STT streaming providers

**Problem:** Several small helper functions are copy-pasted identically (or near-identically) across multiple STT provider files:

- `f32_to_pcm_s16le(samples: &[f32]) -> Vec<u8>` — duplicated in `elevenlabs.rs`, `fireworks.rs`, `openai.rs`, `assemblyai.rs`, `speechmatics.rs`, `deepgram.rs`
- `resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32>` — duplicated in `fireworks.rs`, `openai.rs`
- `decode_to_pcm_s16le_mono(wav_bytes: &[u8]) -> Result<Vec<u8>>` — in `elevenlabs.rs` (WAV decoding for batch path)

**Why it matters:** Any bug fix or improvement (e.g. better resampling quality, clipping protection) needs to be applied to every copy individually. This already bit us during the code review when bugs were found in one provider but not another.

**Suggested fix:** Move these into `app/src-tauri/src/stt/streaming.rs` (which already hosts shared WS and chunking helpers) and have all providers import from there.

**Related:** The "Centralize WebSocket connectivity" refactor item above already added `streaming.rs` — this extends it with audio conversion helpers.

## Revisit provider-family seams only with a real two-adapter proof

**Status:** Deferred by Spec Kit architecture-deepening work (2026-05-03)

The provider-family pre-flight reviewed four possible seams and intentionally did **not** add new production abstractions where they would be pass-through or lossy:

- Managed-mode adaptation already has a load-bearing seam in `pipeline/config.rs::resolve_provider_mode(...)`.
- STT error classification already centralizes retry policy in `stt/retry.rs`, while adapters still need provider-specific response parsing.
- Request metadata/redaction already centralizes sanitization and stripping in `request_log.rs`.
- Cost reporting shares aggregation in `stats.rs` / `commands/stats.rs`, while pricing tables and formulas remain provider-specific.

Reopen a provider-family seam only when at least two concrete adapters can share behavior without erasing provider-specific semantics, and when deleting the seam would clearly reintroduce duplicated caller complexity.

Reference: `specs/017-architecture-deepening-plan/validation/provider-family-decisions.md`.

## Deepen Local Provider Lifecycle ownership beyond helper rules

The Spec Kit architecture-deepening slice extracted deterministic Local Whisper rules into `app/src-tauri/src/pipeline/local_provider_lifecycle.rs`, but `pipeline.rs` still owns the mutable cache operations and UI-facing load/unload commands because those operations need `PipelineInner` state and provider construction.

Future cleanup could introduce a small cache-controller seam around:

- loaded-cache checks
- explicit unload/retain behavior
- force-load provider construction
- config-change eviction decisions

Keep actual model loading explicit and command-driven; do not hide heavy local model loads behind generic provider resolution.

## Narrow Profile Resolution's OCR-mode interface

`app/src-tauri/src/pipeline/profile_resolution.rs` now owns profile matching and effective behavior, but callers still request rewrite, Quick Replace, and Quick Ask OCR modes separately in a few places.

Future cleanup could return a single `ResolvedActiveWindowOcrModes` value for an active/default/global profile context, so callers do not repeat the same precedence wiring and can ask flow-specific questions like "should this session auto-start OCR?" from one resolved object.

## Deepen Quick Ask / Quick Replace request ownership

**Status:** Completed (2026-05-05)

Quick Ask / Quick Replace request ownership is now centralized enough for the original pain point:

- Pure lifecycle vocabulary/config/context decisions live in `app/src-tauri/src/sessions/quick_action_lifecycle.rs`.
- Side-effectful execution ownership lives in `app/src-tauri/src/sessions/quick_action_execution.rs`: context collection, provider readiness, request-log/cost bookkeeping, OCR cleanup, Quick Ask answer emission, and Quick Replace rewrite attempts.
- `app/src-tauri/src/lib.rs::stop_recording(...)` now delegates Quick Ask/Quick Replace execution and keeps the normal dictation output/paste decision visible in the stop-recording path.

Remaining smaller cleanup ideas, if this area gets touched again:

- Consider moving normal dictation final output/paste handling into its own small helper once there is a clear behavior boundary and test coverage.
- Keep expanding pure tests around lifecycle decisions instead of unit-testing Tauri/AppHandle side effects directly.

Keep this separate from provider-family seams. The shared behavior here is about feature request ownership, not generic provider behavior.
