# DRY_REPORT.md — DRY findings (Kolboo)

This is a practical “what should we extract?” report.

It was generated using a **simple duplicate scan + manual review** (no AST parsing, no embeddings). See `DRY_PLAN.md` for the exact command used.

## Summary (where duplication clusters)

The most meaningful duplication is in:

1. **Backend provider plumbing (Rust)** — repeated HTTP request setup, timeouts, error mapping, request/response logging, and response parsing across:
	 - `app/src-tauri/src/llm/**`
	 - `app/src-tauri/src/stt/**`
	 - `app/src-tauri/src/embeddings/**`

2. **Settings UI patterns (TS/TSX)** — repeated “select with Default + hint” rendering blocks across settings screens.

3. **Small math helpers in cost code (Rust)** — token-to-cost conversion helpers are duplicated.

## Ranked duplicate groups (top 10)

Ranking criteria: (a) repeat count, then (b) size of block, then (c) importance/core runtime impact.

### 1) LLM providers: OpenAI-style request/timeout/error/log/parse loop

**What it does:** Send a JSON request to an LLM endpoint, apply optional timeout, map network/timeout errors, parse non-2xx errors into “provider API error”, log request/response JSON, extract text.

**Where (examples / evidence):**

- `app/src-tauri/src/llm/openai.rs` (e.g. around `[402:5 - 468:3]`, `[434:13 - 467:4]`)
- `app/src-tauri/src/llm/groq.rs` (e.g. around `[67:5 - 151:26]`, `[156:26 - 173:14]`)
- Additional similar blocks were detected across other providers (e.g. `anthropic.rs`, `fireworks.rs`, `ollama.rs`, `cohere.rs`, `cerebras.rs`, `gemini.rs`).

**Representative snippet (shape):**

From `app/src-tauri/src/llm/groq.rs` (approx lines ~80-170):

> build request → log request_json → build req (`.post(...)`, `.bearer_auth(...)`, `.json(&request)`) → apply timeout → `send().await` with timeout mapping → `status` check → parse error body (OpenAI-compatible) → parse JSON → log response_json → extract first choice text

**How many times it appears:** at least **6+** LLM provider modules contain the same “request lifecycle” structure.

**Recommendation:** Extract a shared helper for the “send + timeout + error mapping + request/response logging” part.

- Proposed helper module: `app/src-tauri/src/http/openai_style.rs` (or extend existing `openai_compat`)
- Proposed helper signature (example):
	- `async fn send_openai_style_json<Request: Serialize>(client: &Client, provider: &'static str, url: &str, api_key: Option<&str>, timeout: Option<Duration>, request: &Request, log: Option<&RequestLogStore>) -> Result<serde_json::Value, ProviderHttpError>`
- Variable parameters:
	- provider name (for error prefixes + logs)
	- URL / base URL
	- whether auth is bearer token vs none
	- timeout semantics
	- response “extract text” logic (keep provider-specific)

**Risks / gotchas:**

- Providers differ subtly (some are “OpenAI chat completions”, some are “Responses API”, some use slightly different error schema). Keep the helper focused on request lifecycle + generic error parsing; keep response extraction provider-specific.

---

### 2) STT providers: multipart transcription request + consistent error/log handling

**What it does:** Build a multipart form (`wav_transcription_form`), send to a transcription endpoint, map timeout/network errors, parse non-2xx responses, log request/response JSON, extract `text` from JSON.

**Where (examples / evidence):**

- `app/src-tauri/src/stt/openai.rs` `[124:5 - 165:16]` and `[120..260]` shows request logging + send + status check + JSON parsing
- `app/src-tauri/src/stt/groq.rs` `[80..200]` shows nearly identical flow
- `app/src-tauri/src/stt/whisper_server.rs` `[177:36 - 191:2]` (very similar send/status/json/log)
- Additional similar blocks detected in `stt/fireworks.rs`, `stt/elevenlabs.rs`, `stt/deepgram.rs`, `stt/assemblyai.rs`, `stt/aquavoice.rs`.

**How many times it appears:** the same flow shows up across **5–8** STT provider implementations.

**Recommendation:** Extract a helper for the common “transcribe WAV via OpenAI-compatible multipart endpoint” path.

- Proposed helper: `openai_compat::transcribe_wav_multipart(...)` (or `stt/openai_style.rs`)
- Proposed signature (example):
	- `async fn transcribe_wav_multipart(client: &Client, provider: &'static str, endpoint: &str, api_key: Option<&str>, model: &str, audio: &[u8], prompt: Option<&str>, log: Option<&RequestLogStore>) -> Result<String, SttError>`
- Variable parameters:
	- provider name
	- endpoint URL
	- auth strategy
	- model/prompt rules

**Risks / gotchas:**

- Prompt clamping is model/provider-specific (keep that logic per provider; pass final prompt into helper).

---

### 3) Embeddings providers: identical OpenAI-compatible embeddings request/parse

**What it does:** POST JSON `{ model, input }` to `/embeddings`, parse OpenAI-style error response, parse embeddings response, return first embedding vector; also has “debug” mode building redacted request/response JSON.

**Where (evidence):**

- `app/src-tauri/src/embeddings/openai.rs` (see `embed_text_with_url` + `embed_text_with_debug`)
- `app/src-tauri/src/embeddings/fireworks.rs` (nearly identical functions and flow)

**Representative snippet:**

Both files contain the same pattern:

> `.post(url).bearer_auth(api_key).json({model,input}).send()` → `!status.is_success()` parse error → parse `EmbeddingsResponse` → return first `.embedding` → validate non-empty

**How many times it appears:** at least **2** (OpenAI + Fireworks), and it’s the kind of logic that will likely grow as more OpenAI-compatible embedding providers are added.

**Recommendation:** Extract shared code into a single “OpenAI-compatible embeddings” helper.

- Proposed module: `app/src-tauri/src/embeddings/openai_compat.rs`
- Suggested split:
	- one shared “happy path” parser + request builder
	- provider modules only define base URL + error type wrapper

**Risks / gotchas:**

- Error response types differ slightly (`OpenAiErrorResponse` vs `OpenAiCompatErrorResponse`). Consider normalizing the parsed shape in the shared helper, or parse into `serde_json::Value` and pull `error.message` defensively.

---

### 4) Windows key chord injection logic repeated (clipboard/paste + selection probe)

**What it does:** On Windows, inject key chords using Enigo with scancodes, delays, and a “always release modifiers” safety pattern.

**Where (evidence):**

- `app/src-tauri/src/text/inject.rs` (paste via Ctrl+V scancode path)
- `app/src-tauri/src/text/selection_probe.rs` (copy via Ctrl+C / Ctrl+Shift+C / Ctrl+Insert)

**Representative snippet:**

From `selection_probe.rs` (approx lines ~190-280):

> `with_pressed_key(..., Key::Control, |enigo| { enigo.raw(SCANCODE_C, Press); sleep; enigo.raw(SCANCODE_C, Release); sleep; Ok(()) })` + safety release

**How many times it appears:** at least **2** modules share the same “press modifiers → raw scancode click → release → sleep → best-effort modifier release” structure.

**Recommendation:** Extract a small helper for “Windows chord injection with safety”.

- Proposed module: `app/src-tauri/src/text/key_inject.rs`
- Proposed helper: `fn windows_press_scancode_with_modifier(enigo: &mut Enigo, modifier: Key, scancode: u16) -> Result<(), String>`

**Risks / gotchas:**

- Key injection is finicky; keep the helper tiny and well-commented, and don’t change timings in the process.

---

### 5) UI settings: repeated HintSelect “Default + hint” rendering blocks

**What it does:** Render a select where “Default” shows a secondary hint (effective value), and non-default shows just the option label.

**Where (evidence):**

- `app/src/components/settings/ProvidersSettings.tsx` (OpenAI thinking, Gemini thinking level, Gemini thinking budget)
- `app/src/components/settings/prompt/RewriteSettingsSection.tsx` (same patterns, plus inheritance handling)

**Representative snippet:**

Both files contain very similar `renderSelected` / `renderOption` blocks:

> if option is Default → show `option.label` plus `· {hint}`; else show label only

**How many times it appears:** at least **5+** occurrences across these settings screens.

**Recommendation:** Extract a small reusable component or helper.

- Proposed component: `HintSelectWithDefaultHint`
- Suggested props:
	- `data`, `value`, `onChange`, `placeholder`, `defaultHint: string`, plus styling props
- For the rewrite/profile case, add optional `inheritIndicator` slot rather than duplicating the `SettingsInheritanceIndicator` wrapping.

**Risks / gotchas:**

- Be careful not to over-generalize; keep it focused on the “Default-with-hint” rendering only.

---

### 6) Rewrite settings section: repeated SettingsRow scaffolding for “Thinking” variants

**What it does:** Multiple settings rows share the same structure:

> `SettingsRow` label/description → optional inheritance indicator → `HintSelect` with identical styling and default-handling

**Where (evidence):**

- `app/src/components/settings/prompt/RewriteSettingsSection.tsx` (OpenAI thinking, Gemini thinking level, Gemini thinking budget, Anthropic thinking, etc.)

**How many times it appears:** multiple times in one file (jscpd found large intra-file clones).

**Recommendation:** Extract a “row + select” component.

- Proposed component: `SettingsHintSelectRow`
- Signature idea:
	- props: `label`, `description`, `data`, `value`, `onChange`, `defaultHint`, `inheritance?: { ... }`

**Risks / gotchas:**

- This is UI-only refactor; ensure it doesn’t accidentally change default value semantics (especially where `null` has meaning).

---

### 7) Settings editor/test UI: repeated “textarea + ctrl/cmd+enter to run + duration display”

**What it does:** A common UI pattern for test inputs:

> textarea input → keyboard shortcut handler → button that triggers async action → duration label → show error/output

**Where (evidence):**

- `app/src/components/settings/prompt/PresetEditorModal.tsx` contains this pattern (and likely similar patterns elsewhere in settings).

**How many times it appears:** at least **2** (jscpd found large intra-file clones in similar modal code).

**Recommendation:** Extract a small component like `AsyncTestRunnerPanel`.

---

### 8) Tests: repeated scaffolding (low priority)

**What it does:** repeated test setup/fixtures.

**Where (evidence):**

- `app/src-tauri/src/pipeline/tests.rs` has multiple repeated blocks (jscpd flagged many clones)
- Several `.test.ts` files under `app/src/lib/contracts/schemas/` were flagged

**How many times it appears:** many occurrences, but mostly in tests.

**Recommendation:** Optional. Only refactor test duplication if it improves readability without making the tests “too clever”.

---

## Quick-win checklist (lowest-risk refactors first)

1. **Extract OpenAI-compatible embeddings helper** (OpenAI + Fireworks).
2. **Extract a tiny UI helper/component** for “Default-with-hint” select rendering.

## “Do not refactor” list (duplication that’s OK / risky)

- The long list of explicit Tauri `invoke(...)` wrappers in `app/src/lib/tauri/commands.ts` is repetitive, but it also serves as a clear UI↔backend contract. DRY-ing it too much can hide argument shapes and make refactors harder.
- Provider implementations often *look* similar but have important differences (timeouts, auth headers, endpoints, request schemas, error schemas). Prefer extracting only the truly shared lifecycle bits (request building/logging/timeout/error mapping) rather than forcing full inheritance.
- Key injection code is easy to break: extract helpers only if you can keep behavior identical and have good regression coverage.
- Test duplication is sometimes intentional and can keep scenarios readable.
