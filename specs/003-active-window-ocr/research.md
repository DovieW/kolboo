# Phase 0 Research: Active Window OCR Context

This document resolves all open questions from the implementation plan’s Technical Context and records the key technical decisions.

## Decision 1: OCR service protocol (OpenAI-compatible Chat Completions)

**Decision:** Treat OCR as an OpenAI-compatible HTTP service (vLLM) using `POST /v1/chat/completions` with an `image_url` content part.

**Rationale:**
- The user’s target model (`lightonai/LightOnOCR-1B-1025`) is already served via vLLM’s OpenAI-compatible API.
- Kolboo already uses `reqwest` and has patterns for provider HTTP calls.
- This keeps OCR “provider” configuration simple: base URL + fixed model id.

**Alternatives considered:**
- Tesseract/local OCR library: rejected because it doesn’t match the user’s chosen model + adds heavy native deps.
- Custom OCR REST endpoint shape: rejected; OpenAI-compatible makes swapping models/services easier.

## Decision 2: OCR provider configuration surface

**Decision:** Add an OCR provider entry in the existing **Settings → Providers** tab that stores a base URL (string) in `settings.json`.

**Rationale:**
- Matches existing patterns: `whisper_server_base_url` (STT) and `ollama_url` (LLM).
- Lets us hide/disable OCR provider options until the URL is configured (keeps UI clean).

**Alternatives considered:**
- Put OCR under “Advanced” settings: rejected; user explicitly requested Providers tab.
- Use only a checkbox + URL field without a provider concept: rejected; “provider” is consistent with other integrations.

## Decision 3: Per-tool enablement keys

**Decision:** Store three independent booleans in `settings.json`:
- `rewrite_include_active_window_ocr_context`
- `quick_replace_include_active_window_ocr_context`
- `quick_ask_include_active_window_ocr_context`

Defaults: `false`.

**Rationale:**
- Matches the spec (FR-001) and existing “per feature toggle” patterns like clipboard context toggles.
- Keeps behavior predictable and avoids surprising capture.

**Alternatives considered:**
- A single global toggle: rejected (fails FR-001).
- Per-profile toggles only: rejected for initial implementation; higher complexity and less discoverable.

## Decision 4: Active window capture implementation (Windows-first)

**Decision:** Implement active-window screenshot capture in the Rust backend on Windows using Win32 APIs:
1. Find active window with `GetForegroundWindow` + `GetWindowRect`.
2. Capture using `PrintWindow` into a memory DC.
3. If `PrintWindow` fails/returns blank, fall back to `BitBlt` from the screen DC for the window rectangle.

**Rationale:**
- Windows is the primary target platform.
- Avoids JS/webview limitations and keeps screenshot bytes off the UI layer.
- `PrintWindow` can capture the window even if partially occluded; fallback `BitBlt` covers cases where apps don’t implement `WM_PRINT` well.

**Alternatives considered:**
- DXGI Desktop Duplication: rejected for v1 (more complex + GPU-specific edge cases).
- Full-screen capture + crop only: rejected as a primary path; more privacy-invasive and less precise.

## Decision 5: Image encoding + preprocessing

**Decision:** Encode capture as PNG and downscale so the longest dimension is capped at **1540px** (preserving aspect ratio) before sending to OCR.

**Rationale:**
- Model guidance recommends ~1540px longest dimension for best speed/quality tradeoff.
- Smaller images reduce base64 payload size and token cost.

**Alternatives considered:**
- No resizing: rejected; can be slow and produce huge prompts.
- JPEG: possible, but PNG preserves crisp text edges; choose PNG first.

## Decision 6: OCR prompt template + output handling

**Decision:** Use a short, explicit OCR instruction and label the result in the LLM prompt as:

> OCR context from the currently active window:

OCR text is truncated to a safe cap (e.g. 8,000–12,000 chars) with an explicit “… (truncated)” suffix.

**Rationale:**
- The spec requires clear labeling (FR-003) and manageable prompt size (FR-007).
- Truncation is consistent with existing clipboard/selection context logic.

**Alternatives considered:**
- Summarize OCR output using another LLM call: rejected for v1 (extra latency + complexity + more tokens).

## Decision 7: Privacy + logging

**Decision:**
- Do **not** persist screenshots.
- Do **not** log base64 image payloads.
- Prefer not to persist raw OCR text in request logs; store only boolean/char-count metadata indicating OCR context was present.

**Rationale:**
- OCR can accidentally capture sensitive information visible on screen.
- The spec requires minimizing sensitive data exposure by default (FR-008).

**Alternatives considered:**
- Store truncated OCR text in logs: rejected for v1; too risky.

## Decision 8: Deterministic testing strategy

**Decision:** Make OCR integration testable without real network calls by:
- Isolating OCR HTTP calls behind a small client interface.
- Using `wiremock` in Rust tests to simulate `/v1/chat/completions` responses.
- Unit-testing prompt construction and truncation logic as pure functions.

**Rationale:**
- Satisfies the repo constitution: deterministic tests with no real network/services.

**Alternatives considered:**
- End-to-end tests hitting `http://localhost:8000`: rejected (non-deterministic, requires local server).

## Decision 9: Start OCR as early as possible (parallel with transcription)

**Decision:** When OCR context is enabled, start active-window capture + OCR request immediately when the user triggers the tool (i.e., at the start of the pipeline/session), and let it run concurrently with STT recording/transcription.

**Rationale:**
- OCR is “nice-to-have” context; the user shouldn’t feel extra wait time.
- We can overlap OCR latency with time spent recording + running STT.

**Implementation sketch:**
- Spawn an async task (or background worker) that captures the active window and calls OCR.
- Later, when constructing the LLM prompt, either:
	- attach OCR text if it’s ready, or
	- continue without it (bounded by a small timeout).

**Alternatives considered:**
- Only start OCR after transcription completes: rejected; adds avoidable user-visible latency.

## Decision 10: Tri-state per-tool setting (off / auto / manual)

**Decision:** Replace the per-tool boolean “include OCR context” toggle with a 3-state mode:

- `off`: never run OCR
- `auto`: start OCR immediately when the tool starts (in parallel with transcription)
- `manual`: do not start OCR automatically; instead, show a new OCR button in the **recording overlay** (to the right of the waveform). OCR starts immediately when the button is clicked.

**Cancellation behavior:**

- If OCR is running (auto or manual) and the user switches the mode to `off`, cancel the in-flight OCR task and proceed without OCR context.

**Rationale:**

- Manual mode gives the user a privacy/intent checkpoint: OCR only happens when explicitly clicked.
- Auto mode remains the fastest path for users who want zero extra steps.

**Alternatives considered:**

- Only support on/off: rejected; doesn’t meet the desired UX.

## Decision 11: Provider-agnostic OCR settings (URL + model + optional auth)

**Decision:** Add OCR provider settings that allow Kolboo to target multiple OpenAI-style OCR-capable services, not just LightOnOCR.

Minimum configurable fields:

- Base URL
- Model id
- Optional auth mode + API key (stored securely)

**Rationale:**

- Users may want to swap OCR models/providers over time.
- vLLM local today, hosted provider tomorrow should be a settings change, not a code change.

**Security note:**

- API keys must be stored in OS secure storage (keyring) and never logged.

## Decision 12: Robust OCR lifetime via session-owned OCR jobs (Option A)

**Decision:** Move OCR task/result ownership out of the pipeline’s generic state transitions and into an explicit per-request `SessionContext` with a stable `session_id`.

In other words: OCR should be a “child job” of the user’s current tool invocation (Quick Ask / Rewrite / Quick Replace), not a side-effect that can be cleared by unrelated pipeline resets.

**Rationale:**

- Real logs showed “OCR: started” followed by “Quick Ask: proceeding without OCR (status=not_started)”.
- Root cause: `reset_to_idle()` (and some cancel paths) clear OCR state, so later “wait up to timeout” code has nothing to wait on.
- This failure mode can affect **all** OCR-consuming flows, not just Quick Ask.

**What this enables:**

- A tool can confidently wait up to `ocr_request_timeout_ms` *if and only if* OCR is running for the same session.
- Cancelling a session (Escape, superseded by a new session, force reset) cancels OCR with an explicit reason.
- Future OCR features can add new OCR purposes/jobs without colliding with existing ones (e.g., multiple jobs keyed by purpose).

**Alternatives considered:**

- “Easy fix”: stop clearing OCR state in `reset_to_idle()`.
	- Rejected for long-term robustness: any future recovery/reset path might still clear OCR inadvertently, and it’s hard to reason about which state transitions should or should not destroy OCR.

**Contract implications:**

- Add `session_id` and a stable OCR job status into overlay polling/state so UI actions (manual trigger) can target the active session.
- Ensure Rust↔TS command/event/type updates stay in sync.
