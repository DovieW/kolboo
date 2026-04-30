---
name: providers-models
description: Instructions on working with providers and models in this app
---

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

Other providers also live under:

- `app/src-tauri/src/cost/<provider>.rs`

If you want the UI to show a per-model price (e.g. “$X / hour” for STT or “$X / 1M tokens” for LLM), wire it through the pricing command:

- Backend: `app/src-tauri/src/commands/pricing.rs` (`get_model_pricing`)
- Frontend: `app/src/lib/queries.ts` (`useModelPricing`) + `app/src/lib/tauri.ts` (`statsAPI.getModelPricing`)

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

#### Adding a new free-tier toggle (end-to-end)

Free-tier is a **settings-store boolean** (e.g. `${provider}_free_tier`) that affects **stats/logging**, not provider availability.

Backend (Rust):

- Seed a default value for missing keys in:
  - `app/src-tauri/src/lib.rs` → `ensure_default_settings`
  - (This runs on startup so UI and backend agree on effective defaults.)
- Tag the cost event as free-tier in:
  - `app/src-tauri/src/stats.rs` (look for the centralized `is_free_tier_call` logic)
- Keep estimation behavior consistent:
  - If free-tier should still show “list price estimate”, emit both `is_free_tier = true` and a non-zero `estimated_cost_usd_micros` (UI can filter).
  - If free-tier should mean “$0”, set `estimated_cost_usd_micros = Some(0)`.

Frontend (TS/React):

- Add the setting to the typed schema:
  - `app/src/lib/tauri.ts` (`AppSettings` + normalization default)
- Add a settings updater:
  - `app/src/lib/tauri.ts` (e.g. `update<Provider>FreeTier(enabled)`)
- Add a React Query mutation that updates the store and syncs pipeline config:
  - `app/src/lib/queries.ts`
- Add the toggle UI:
  - Usually `app/src/components/settings/ApiKeysSettings.tsx` (for “account/billing” toggles)

---

## Mental model (how settings flow)

On app startup, the backend seeds missing defaults into `settings.json` via:

- `app/src-tauri/src/lib.rs` → `ensure_default_settings`

This prevents “missing key” mismatches between UI defaults and backend runtime fallbacks.

There are two backend “read settings” paths to keep in mind:

- Startup: `app/src-tauri/src/lib.rs` → `initialize_pipeline_from_settings`
- After UI changes: `app/src-tauri/src/commands/config.rs` → `sync_pipeline_config`

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

Also check for **hardcoded maintenance lists** that should include your new provider id/key:

- `app/src-tauri/src/commands/data.rs` → `delete_all_api_keys` (known `*_api_key` keys)

If these drift, the provider will silently disappear from dropdowns (because it’s filtered by “has API key”).

---

## Adding a provider-specific setting (toggle/knob)

If your provider needs **any extra setting** beyond `provider`, `model`, and `${provider}_api_key` (examples: “free tier”, “thinking budget”, “endpoint URL”), treat it as a first-class Store setting.

Minimum plumbing checklist:

1. **Frontend type + default**

    - `app/src/lib/tauri.ts` → extend `AppSettings` and ensure missing keys get a sensible default during `getSettings()`.

2. **Frontend updater**

    - `app/src/lib/tauri.ts` → add an `updateX(...)` function that writes to the Store.

3. **Frontend mutation**

    - `app/src/lib/queries.ts` → add a `useUpdateX(...)` mutation that calls the updater and then `configAPI.syncPipelineConfig()`.

4. **UI control**

    - Add the component to the appropriate settings screen.
      - Account/provider toggles often live in `app/src/components/settings/ApiKeysSettings.tsx`.
      - Runtime knobs often live in `app/src/components/settings/ProvidersSettings.tsx` or `PromptSettings.tsx`.

5. **Backend default seeding / migration**

    - `app/src-tauri/src/lib.rs` → `ensure_default_settings`.

6. **Backend consumption**

    - Read the setting in `app/src-tauri/src/commands/config.rs` (`sync_pipeline_config`) and/or
      `app/src-tauri/src/lib.rs` (`initialize_pipeline_from_settings`) and plumb into `PipelineConfig`.

---

## Request logs (Logs UI) and provider payload debugging

Providers can enrich the active request log with request/response payloads. This is extremely helpful for provider integrations (especially WS/streaming).

Key points:

- The request log store is managed as app state:
  - `app/src-tauri/src/request_log.rs` (`RequestLogStore`)
  - wired up in `app/src-tauri/src/lib.rs`
- The pipeline passes an optional store into providers:
  - LLM providers commonly implement `with_request_log_store(...)` (see `app/src-tauri/src/pipeline.rs` → `create_llm_provider`)
  - STT providers should follow the same pattern so they can attach:

    - request JSON
    - response JSON
    - intermediate debugging events

If you’re adding a provider and nothing shows up in Logs:

- Verify the store is being preserved across config sync:
  - `app/src-tauri/src/commands/config.rs` keeps `request_log_store` on the new `PipelineConfig`.
- Verify your provider accepts the store and writes to it.

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

- saves the key through `tauriAPI.setApiKey` to OS secure storage (legacy `settings.json` fallback is read/migration-only)
- invalidates `availableProviders`
- calls `configAPI.syncPipelineConfig()`

### 8) Frontend: add model options

Edit:

- `app/src/lib/modelOptions.ts`

Add a new `LLM_MODELS["<id>"] = [...]` entry.

Ordering matters:

- when a user switches providers, the UI resets the model to `LLM_MODELS[id][0]`
- so put your _recommended default_ first

### 8a) Scaling to “too many models”: fetch dynamically (on the fly)

Some providers (example: Fireworks) have **large and fast-changing catalogs** where hardcoding model IDs is both annoying and fragile.

Pattern we use in this repo:

1. **Backend exposes a Tauri command** that returns model options as plain data (`Vec<ModelOption>`).
2. **Frontend queries it** via React Query, and uses it as a drop-in replacement for the static `LLM_MODELS[provider]` array.
3. **Fallback**: if dynamic listing fails (offline, API error), the UI can fall back to a small curated list.

#### Fireworks implementation (reference)

Backend (Rust):

- Command: `fireworks_list_models`
  - File: `app/src-tauri/src/commands/fireworks.rs`
  - Exported from the command list in: `app/src-tauri/src/lib.rs`
- Return type is the same shape the UI already expects for a picker:
  - `ModelOption { value: String, label: String, disabled: bool }`

Frontend (TS/React):

- Invoke wrapper:
  - `app/src/lib/tauri.ts` → `llmAPI.getFireworksModels()` → `invoke<ModelOption[]>("fireworks_list_models")`
- Query hook:
  - `app/src/lib/queries.ts` → `useFireworksModels(enabled)`
- UI usage:
  - `app/src/components/settings/PromptSettings.tsx`
    - When the selected provider is `"fireworks"`, it prefers the dynamic list when available.

#### Important: list only callable models (avoid “catalog-only” 404s)

Some providers expose separate endpoints for:

- “Catalog metadata” (may include models you cannot call with your API key / not deployed)
- “Inference models” (models that are actually callable)

If you offer catalog-only entries in the picker, the user can select a model ID that later fails with a 404-style error ("not found/inaccessible/not deployed").

What we do for Fireworks:

- Treat the inference list (`/inference/v1/models`) as the **authoritative set of callable model IDs**.
- Optionally fetch catalog metadata only to improve labels, but **do not** show catalog-only models in the UI.
- Filter out models that are not relevant to the picker (e.g. rerank or image/diffusion models) so the UI doesn’t present non-chat targets.

#### Caching: make dynamic model pickers feel instant

Dynamic listing can be called frequently (settings screens, Quick Ask, router configs), so we cache results.

Fireworks uses:

- **In-memory TTL cache** (fast path)
- **Disk cache** under the app data dir (survives restarts)
  - Location: `<app_data_dir>/cache/fireworks-models.json`
  - Keying: cache is keyed by an **API key fingerprint hash** (never store the raw API key)
  - TTL: keep it reasonably long on disk to avoid rate limits / slow settings loads

This is implemented in `app/src-tauri/src/commands/fireworks.rs`.

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

#### Example: ElevenLabs STT (Scribe)

ElevenLabs supports file-based speech-to-text via:

- `POST https://api.elevenlabs.io/v1/speech-to-text`
- header: `xi-api-key: <key>`
- multipart form fields:
  - `model_id` (currently `scribe_v2` and `scribe_v1`; `scribe_v2` also supports realtime streaming)
  - `file=@audio.wav`

The synchronous response includes a top-level `text` string.

Notes:

- This endpoint does **not** currently support the OpenAI-style `prompt` field, so treat it as **no-prompt** unless/ until ElevenLabs adds an equivalent parameter.
- Pricing on ElevenLabs is primarily **credit-based**. Unless the repo defines a stable USD/minute mapping, Stats/Logs may record STT events without `estimated_cost_usd_micros`.

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
- [ ] If the provider has extra settings: added Store setting plumbing + `ensure_default_settings` seed
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
- [ ] If the provider has extra settings (free-tier, endpoints, etc): added Store setting plumbing + `ensure_default_settings` seed
- [ ] If you added a new `*_api_key`: updated `delete_all_api_keys` (`app/src-tauri/src/commands/data.rs`)
