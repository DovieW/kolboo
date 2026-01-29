# Data Model: Active Window OCR Context

This feature is primarily **settings-driven** (persisted to `settings.json`) plus **ephemeral runtime data** (captured screenshot bytes + OCR text) used only during a single tool invocation.

## Settings (persisted)

### OCR provider configuration

| Key | Type | Default | Notes |
|---|---:|---:|---|
| `ocr_base_url` | `string \| null` | `null` | Base URL for the OCR service. Examples: `http://localhost:8000` (vLLM), `https://api.openai.com` (OpenAI). When `null`/empty/invalid, OCR provider is treated as unavailable. |
| `ocr_model` | `string \| null` | `"lightonai/LightOnOCR-1B-1025"` (effective) | OCR model identifier. Must be configurable to support other OCR-capable models. |
| `ocr_auth_mode` | `"none" \| "bearer_api_key" \| null` | `"none"` (effective) | How requests are authenticated. `bearer_api_key` means `Authorization: Bearer <key>`. |
| `ocr_request_timeout_ms` | `number \| null` | `2000` (effective) | Optional. Keep small so OCR never blocks the tool. |

### Per-tool OCR context mode (tri-state)

Each tool can be configured independently with a 3-state mode:

- `"off"`: never run OCR
- `"auto"`: run OCR automatically when the tool is triggered
- `"manual"`: show an OCR button in the recording overlay; OCR runs only after the button is clicked

| Key | Type | Default | Applies to |
|---|---:|---:|---|
| `rewrite_active_window_ocr_mode` | `"off" \| "auto" \| "manual" \| null` | `"off"` (effective) | Rewrite tool LLM step |
| `quick_replace_active_window_ocr_mode` | `"off" \| "auto" \| "manual" \| null` | `"off"` (effective) | Quick Replace tool |
| `quick_ask_active_window_ocr_mode` | `"off" \| "auto" \| "manual" \| null` | `"off"` (effective) | Quick Ask tool |

**Migration note:** if older settings used booleans like `*_include_active_window_ocr_context`, migrate as:

- `true` → `"auto"`
- `false`/missing → `"off"`

### Prompt size guardrails

| Key | Type | Default | Notes |
|---|---:|---:|---|
| `ocr_context_max_chars` | `number \| null` | `8000` (effective) | Maximum characters of OCR output included in downstream LLM prompts. |

## Ephemeral runtime objects (not persisted)

### `SessionContext` (per tool invocation)

This feature is best modeled as per-request/session data. A session owns all transient work that must remain coherent across pipeline state transitions.

- `session_id: String` (UUID)
- `flow: "quick_ask" | "rewrite" | "quick_replace" | <future>`
- `created_at: DateTime<Utc>`
- `cancelled_at: Option<DateTime<Utc>>`
- `cancel_reason: Option<String>` (sanitized, user-friendly)
- `ocr_jobs: Map<OcrPurpose, OcrJob>`

`SessionContext` lifetime rules:

- Created when a tool session starts.
- Destroyed only when the tool finishes or is explicitly cancelled/superseded.
- Pipeline state transitions (Idle/Recording/Transcribing/…) should not implicitly clear `SessionContext`.

### `OcrPurpose`

Keyed identifier for OCR jobs within a session:

- `active_window_context` (current feature)
- (future) `selected_region`
- (future) `document_page`

### `OcrJob`

An OCR job tracked within the session.

- `purpose: OcrPurpose`
- `mode: "auto" | "manual"` (how it was started)
- `status: "not_started" | "running" | "done" | "failed" | "cancelled"`
- `started_at: Option<DateTime<Utc>>`
- `finished_at: Option<DateTime<Utc>>`
- `failed_reason: Option<String>` (sanitized)
- `result: Option<OcrResult>`

### `ActiveWindowCapture`

- `captured_at_ms: u64`
- `window_title: Option<String>` (best-effort)
- `image_png_bytes: Vec<u8>`
- `image_width_px: u32`
- `image_height_px: u32`

### `OcrResult`

- `text: String` (already truncated/normalized for prompt inclusion)
- `truncated: bool`
- `provider: "openai-compatible"` (or specific provider id)
- `model: String`

### `OcrTaskState` (manual mode support)

- `status: "not_started" | "running" | "done" | "failed" | "cancelled"`
- `started_by: "auto" | "manual"`
- `cancel_token: CancellationToken`

This lets the recording overlay:

- show an OCR button only when mode is `"manual"`
- show progress/disabled state while running
- cancel the in-flight task when the mode flips to `"off"` or the session ends

## Request log metadata (persisted as part of existing request log model)

We should record only **presence** and **size** information, not raw OCR text:

- `ocr_session_id: Option<String>` (links breadcrumbs to a single session)
- `ocr_status: Option<String>`
- `ocr_context_present: bool`
- `ocr_context_chars: Option<usize>`
- `ocr_failed_reason: Option<String>` (short user-friendly reason; avoid technical stack traces)

## Secure storage

If `ocr_auth_mode` requires an API key:

- Store the OCR API key in the same secure mechanism used by other providers (keyring / OS credential store), not in `settings.json`.
- Suggested secret key name: `ocr_api_key`.
