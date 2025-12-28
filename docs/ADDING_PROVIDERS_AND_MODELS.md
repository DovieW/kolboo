# Adding Providers & Models (LLM + STT)

This repo is a **Tauri (Rust) backend + React/TS frontend** desktop app.

Provider/model changes are “full stack”:

- **Frontend** controls pickers + persists settings into the Tauri Store (`settings.json`).
- **Backend** reads settings from the Store in `sync_pipeline_config` and rebuilds the runtime pipeline.

This guide documents the _exact extension points_.

---

## Pricing / cost estimation (Stats + Logs)

This app has **two cost surfaces**:

1. **Persistent Stats ledger** (used by the Stats UI)
   - Cost events are emitted from:
     - `app/src-tauri/src/stats.rs` (`emit_cost_events_for_current_request`)
   - Events are written as daily JSONL shards under:
     - `<app_data_dir>/stats/cost-events-YYYY-MM-DD.jsonl`
   - Aggregation for the Stats UI happens in:
     - `app/src-tauri/src/commands/stats.rs`

2. **In-memory Request Logs** (used by the Logs UI)
   - The active request log is enriched with per-call fields like:
     - `stt_estimated_cost_usd_micros`, `llm_estimated_cost_usd_micros`
     - `stt_is_free_tier`, `llm_is_free_tier`
   - These fields are set as a side-effect of emitting cost events in `stats.rs`.

### Where pricing tables live

Provider pricing tables and estimation helpers live in:

- `app/src-tauri/src/cost/openai.rs`
- `app/src-tauri/src/cost/groq.rs`

Both modules use **USD micros** (`UsdMicros`):

- $1 USD = 1,000,000 micros

### How estimation works

- **LLM** calls: estimate from token usage in `llm_response_json`.
  - `stats.rs` parses OpenAI-compatible usage fields (chat completions: `usage.prompt_tokens` / `usage.completion_tokens`).
- **STT** calls: estimate from audio duration.
  - `stats.rs` prefers WAV-derived duration (ground truth), and may fall back to provider-reported durations when available.
  - Providers may have special billing rules (example: Groq STT has a **10s minimum billed length**).

### Adding/updating pricing for a provider

When you add a new provider/model (or change model lists), update **both**:

1. Pricing tables/estimators in `app/src-tauri/src/cost/<provider>.rs`.
2. Emission wiring in `app/src-tauri/src/stats.rs` so `CostEvent.estimated_cost_usd_micros` is filled.

If `estimated_cost_usd_micros` is `None`, the event will still be recorded, but it won’t count as “priced” in Stats.

### Free tier / $0 calls

Some providers support marking calls as free-tier (e.g. Groq via the `groq_free_tier` store setting).

- If a call is free-tier, emit a priced event with cost set to `0` micros.
- Stats filtering can then exclude free-tier calls without losing event counts.

---

## Mental model (how settings flow)

1. UI writes to the Store via `app/src/lib/tauri.ts` helpers.
2. React Query mutations in `app/src/lib/queries.ts` call:
   - Store update
   - `configAPI.syncPipelineConfig()` (Tauri command)
3. Rust command `sync_pipeline_config` (in `app/src-tauri/src/commands/config.rs`) reads keys from `settings.json` and updates `PipelineConfig`.
4. `PipelineConfig` drives provider construction in `app/src-tauri/src/pipeline.rs`.

---

## Provider IDs (important)

Pick a single provider id (string), e.g. `"openai"`, `"gemini"`, `"anthropic"`.

That id must match across:

- API key store key: `"${id}_api_key"` (example: `openai_api_key`)
- Available providers list: `app/src-tauri/src/commands/config.rs`
- Frontend model registry keys: `app/src/lib/modelOptions.ts`
- Provider creation matches in Rust pipeline: `app/src-tauri/src/pipeline.rs`
- Any per-provider key aggregation lists inside `sync_pipeline_config`

If these drift, the provider will silently disappear from dropdowns (because it’s filtered by “has API key”).

---

## Add a new LLM provider

### 1) Backend: implement the provider (LLM)

Create a new file:

- `app/src-tauri/src/llm/<your_provider>.rs`

Implement the trait:

- `crate::llm::LlmProvider` (declared in `app/src-tauri/src/llm/mod.rs`)

Follow the established pattern in:

- `app/src-tauri/src/llm/openai.rs`
- `app/src-tauri/src/llm/gemini.rs`
- `app/src-tauri/src/llm/anthropic.rs`

Minimum expectations:

- Validate missing API key → `Err(LlmError::NoApiKey("<id>".to_string()))`
- Respect `timeout: Option<Duration>` (support `.without_timeout()` for settings test actions)
- Make `fn name() -> &'static str` return your provider id
- Make `fn model() -> &str` return the currently configured model string

### 2) Backend: export the module (LLM)

Edit `app/src-tauri/src/llm/mod.rs`:

- add `mod <your_provider>;`
- add `pub use <your_provider>::<YourProviderStructName>;`

### 3) Backend: wire provider creation (LLM)

Edit `create_llm_provider` in:

- `app/src-tauri/src/pipeline.rs`

Add a new `match config.provider.as_str()` arm that constructs your provider.

This is also where provider-specific knobs are applied (examples already present):

- OpenAI: `.with_reasoning_effort(config.openai_reasoning_effort.clone())`
- Gemini: `.with_thinking_budget(config.gemini_thinking_budget)`
- Anthropic: `.with_thinking_budget(config.anthropic_thinking_budget)`

### 4) Backend: add the provider to “available providers” (LLM)

Edit `LLM_PROVIDERS` in:

- `app/src-tauri/src/commands/config.rs`

Add an entry `(id, label, is_local)`.

Notes:

- Cloud providers should have `is_local = false`.
- Local providers should have `is_local = true` (they show up even without an API key).

This list drives the UI dropdown via `configAPI.getAvailableProviders()`.

### 5) Backend: include the API key in the aggregated `llm_api_keys` map (LLM)

In `sync_pipeline_config` (`app/src-tauri/src/commands/config.rs`), there is a section:

- `// Read all available LLM API keys (for per-profile provider overrides at runtime)`

It currently enumerates providers like:

- `for provider in ["openai", "anthropic", "groq", "gemini"] { ... }`

Add your provider id there, otherwise:

- the provider may appear in the UI
- but **per-profile overrides / runtime selection will fail** because the pipeline won’t have the key in `llm_api_keys`.

### 6) Backend: set a default model (LLM)

There are _two_ sources of truth to keep aligned:

1. Your provider implementation’s `DEFAULT_MODEL` constant
2. `default_llm_model_for_provider` in:
   - `app/src-tauri/src/llm/defaults.rs`

When the user has never selected a model, the pipeline uses `defaults.rs` to pick a concrete model for logging and stability.

### 7) Frontend: add API key UI

Edit:

- `app/src/components/settings/ApiKeysSettings.tsx`

Add an entry to `API_KEYS`:

- `id`: your provider id
- `storeKey`: `"${id}_api_key"`
- `getKeyUrl`: wherever users obtain keys

This automatically:

- saves the key to the Store via `tauriAPI.setApiKey`
- invalidates `availableProviders`
- calls `configAPI.syncPipelineConfig()`

### 8) Frontend: add model options

Edit:

- `app/src/lib/modelOptions.ts`

Add a new `LLM_MODELS["<id>"] = [...]` entry.

Ordering matters:

- when a user switches providers, the UI resets the model to `LLM_MODELS[id][0]`
- so put your _recommended default_ first

### 9) Frontend: (optional) provider-specific settings UI

If your provider has special knobs (like “thinking”), you’ll need:

- Store schema additions in `app/src/lib/tauri.ts` (`AppSettings` + normalization + updater)
- React Query mutation in `app/src/lib/queries.ts` that calls the updater and then `syncPipelineConfig`
- UI controls in:
  - `app/src/components/settings/PromptSettings.tsx` (Default profile scope)
  - and/or `app/src/components/settings/ProvidersSettings.tsx`

---

## Add a new STT provider

STT providers follow the same shape, but use the `SttProvider` trait.

### 1) Backend: implement STT provider (STT)

Create:

- `app/src-tauri/src/stt/<your_provider>.rs`

Implement:

- `crate::stt::SttProvider` (declared in `app/src-tauri/src/stt/mod.rs`)

Follow patterns in:

- `app/src-tauri/src/stt/openai.rs`
- `app/src-tauri/src/stt/groq.rs`
- `app/src-tauri/src/stt/deepgram.rs`

### 2) Backend: export module (STT)

Edit `app/src-tauri/src/stt/mod.rs`:

- `mod <your_provider>;`
- `pub use <your_provider>::<YourProviderStructName>;`

### 3) Backend: wire provider creation (STT)

Edit `PipelineInner::get_or_create_stt_provider` in:

- `app/src-tauri/src/pipeline.rs`

Add a `match provider_id.as_str()` arm.

Also ensure the provider id is included in the **API key aggregation** in `sync_pipeline_config`:

- `for provider in ["openai", "groq", "deepgram"] { ... }`

### 4) Backend: add to available providers list (STT)

Edit `STT_PROVIDERS` in:

- `app/src-tauri/src/commands/config.rs`

### 5) Frontend: add API key UI (if cloud)

Edit:

- `app/src/components/settings/ApiKeysSettings.tsx`

### 6) Frontend: add model list (if applicable)

Edit:

- `app/src/lib/modelOptions.ts` → `STT_MODELS["<id>"]`

### 7) Frontend: if your STT supports prompting

Prompting is gated in:

- `app/src/components/settings/PromptSettings.tsx`

Look for:

- `sttPromptSupported`
- prompt max length logic (some providers/models are 224-char limited)

Update those conditions if your new provider/model supports prompting.

---

## Add a new model (existing provider)

### 1) Frontend: expose it in the picker

Edit:

- `app/src/lib/modelOptions.ts`

Add the model to the provider’s array:

- `LLM_MODELS[provider]` or `STT_MODELS[provider]`

**Tip:** Put the best default first (provider switch resets to the first model).

### 2) Backend: ensure the provider can actually use it

Most providers just pass the model string through. But some have per-model feature gates:

- OpenAI reasoning effort / structured outputs gates live in:
  - `app/src-tauri/src/llm/openai.rs`
- Gemini thinking config validation lives in:
  - `app/src-tauri/src/llm/gemini.rs`
- Anthropic extended thinking model allowlist lives in:
  - `app/src-tauri/src/llm/anthropic.rs`

If your new model changes what’s supported, update these checks to avoid 400s.

### 3) Backend: align defaults (only if it’s the new recommended default)

If you want the app’s implicit default to switch, update:

- provider file’s `DEFAULT_MODEL`
- `app/src-tauri/src/llm/defaults.rs` (`default_llm_model_for_provider`)

---

## Structured outputs (don’t forget this)

The app’s rewrite step is much easier to make robust if providers return a tiny JSON object:

```json
{ "rewritten_text": "..." }
```

### How it works today

- **OpenAI** (`app/src-tauri/src/llm/openai.rs`)

  - Uses the Responses API `text.format` with `type: "json_schema"`.
  - Gated by `supports_structured_outputs(model)`.
  - Parses the returned JSON and extracts `rewritten_text`.

- **Gemini** (`app/src-tauri/src/llm/gemini.rs`)

  - Uses `generationConfig.responseMimeType = "application/json"`
  - Uses `generationConfig.responseJsonSchema = <schema>`
  - Parses JSON and extracts `rewritten_text`.

- **Anthropic / Groq / Ollama**
  - Currently **unstructured** (plain text).

### Adding structured outputs to a new provider

Recommended pattern:

1. Define a minimal schema (one field: `rewritten_text`).
2. Add a short system instruction reinforcing “return ONLY valid JSON”.
3. Parse the provider output as JSON.
4. Extract `rewritten_text`.
5. Gate the behavior per-model if needed (some models/APIs reject schema mode).

Where to put the gate:

- Inside the provider implementation, near request building (see OpenAI’s `supports_structured_outputs`).

Why gating matters:

- If you send schema/JSON-mode params to unsupported models, you’ll get 400s.
- The goal is: unsupported model → degrade gracefully to plain text, not a hard failure.

---

## Quick checklist (LLM provider)

- [ ] Added provider implementation file under `app/src-tauri/src/llm/`
- [ ] Exported in `app/src-tauri/src/llm/mod.rs`
- [ ] Wired in `create_llm_provider` (`app/src-tauri/src/pipeline.rs`)
- [ ] Added to `LLM_PROVIDERS` (`app/src-tauri/src/commands/config.rs`)
- [ ] Added to `llm_api_keys` enumeration in `sync_pipeline_config`
- [ ] Added API key UI entry (`app/src/components/settings/ApiKeysSettings.tsx`)
- [ ] Added `LLM_MODELS[provider]` entries (`app/src/lib/modelOptions.ts`)
- [ ] Set default model in both provider `DEFAULT_MODEL` and `llm/defaults.rs`
- [ ] Added/updated pricing tables in `app/src-tauri/src/cost/<provider>.rs` (if Stats/Logs should show cost)
- [ ] Wired cost estimation in `app/src-tauri/src/stats.rs` (emit `estimated_cost_usd_micros`)
- [ ] If structured outputs supported: implemented + gated + parsed

---

## Quick checklist (STT provider)

- [ ] Added provider implementation under `app/src-tauri/src/stt/`
- [ ] Exported in `app/src-tauri/src/stt/mod.rs`
- [ ] Wired in `PipelineInner::get_or_create_stt_provider` (`app/src-tauri/src/pipeline.rs`)
- [ ] Added to `STT_PROVIDERS` (`app/src-tauri/src/commands/config.rs`)
- [ ] Added to `stt_api_keys` enumeration in `sync_pipeline_config`
- [ ] Added API key UI entry (if cloud)
- [ ] Added `STT_MODELS[provider]` entries (if applicable)
- [ ] Updated prompting gates in `PromptSettings.tsx` (if applicable)
- [ ] Added/updated pricing tables in `app/src-tauri/src/cost/<provider>.rs` (if Stats/Logs should show cost)
- [ ] Wired cost estimation in `app/src-tauri/src/stats.rs` (emit `estimated_cost_usd_micros`)
