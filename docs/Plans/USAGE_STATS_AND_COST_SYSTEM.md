# Usage Stats & Cost System (Implementation Guide)

This document explains how the **Usage Stats → Cost** system works end-to-end (backend + frontend), and how to extend it with new **providers/models**, new **cost estimators**, and new **UI features**.

> Goal: make it easy for a new contributor to add cost tracking for a provider/model and surface it in the app.

---

## What exists today

### User-facing UI (current)

- Sidebar page: **Usage Stats**
- First sub-tab: **Cost**
- Current UI:
  - **Total spend** card at the top
  - **Timeframe** dropdown (top-right)

### Data source (current)

- Stats are **persisted on disk** (not derived from in-memory request logs).
- Stats are written opportunistically during transcription flows.

---

## High-level architecture

### Backend (Rust/Tauri)

1. **Transcription happens** (STT + optional LLM formatting).
2. We gather what we can from the current `RequestLog`:
   - provider/model
   - response JSON (when available)
3. We estimate cost:
   - token-based estimation (when response usage fields exist)
   - otherwise STT per-minute pricing using WAV duration
4. We write a `CostEvent` to a persisted ledger (JSONL).
5. Retention pruning runs:
   - on startup
   - after each event write (best-effort)

### Frontend (React)

- The Cost tab uses a lightweight Tauri command (`get_cost_summary`) that returns a single aggregated total for a timeframe.

---

## On-disk storage format

### Stats directory

Stats are stored under your app data dir:

- `<app_data_dir>/stats/`

Inside it, cost events are sharded by day in JSONL:

- `cost-events-YYYY-MM-DD.jsonl`

### JSONL

Each line is a JSON object representing a single `CostEvent`.

**Why JSONL + daily shards?**

- appending is cheap
- pruning is easy (delete old shard files)
- easy to migrate to SQLite later if/when needed

---

## Core backend modules (where to look)

### Stats store + schema

- `app/src-tauri/src/stats.rs`
  - `StatsStore` (append + prune)
  - `CostEvent` schema
  - `StatsRetentionConfig`
  - helpers:
    - `wav_duration_secs(wav_bytes)`
    - `parse_openai_usage_from_response_json(...)`

### Provider pricing + estimators

- `app/src-tauri/src/cost/openai.rs`
  - OpenAI token pricing tables (text/audio)
  - OpenAI transcription per-minute pricing
  - `estimate_cost_from_usage(model, usage)`
  - `estimate_transcription_cost_from_audio_secs(model, secs)`

### Where cost events are written

- `app/src-tauri/src/commands/recording.rs`
  - `write_cost_events_for_current_request(...)`
  - called on:
    - `pipeline_stop_and_transcribe` success/error
    - `pipeline_retry_transcription` success/error

### Aggregation command (used by UI)

- `app/src-tauri/src/commands/stats.rs`
  - `get_cost_summary(timeframe)`

### Initialization + startup pruning

- `app/src-tauri/src/lib.rs`
  - constructs `StatsStore` and `manage(...)`s it
  - prunes using retention settings immediately on startup

---

## Settings + retention

### Backing settings keys

These live in `settings.json` via the store plugin.

- `stats_retention_unit`: `"days" | "hours"`
- `stats_retention_value`: number (0 = keep forever)
- `stats_retention_max_bytes`: number (defensive cap)

Defaults are seeded in:

- `app/src-tauri/src/settings/defaults.rs` (`ensure_default_settings`)

### UI

- `app/src/components/settings/DataSettings.tsx`
  - “Stats retention” row

### Why there is also a `max_bytes` cap

Time retention is not sufficient if clocks change, or if someone sets retention to “forever”. The byte cap is a safety net.

---

## Frontend wiring (current)

### UI entry point

- `app/src/App.tsx`
  - `UsageStatsView()`
  - renders the Cost tab panel

### Cost tab component

- `app/src/components/usageStats/CostTab.tsx`
  - timeframe dropdown
  - top card with total spend

### Frontend API

- `app/src/lib/tauri.ts`
  - `tauriAPI.getCostSummary({ timeframe })`
  - types:
    - `CostTimeframe`
    - `CostSummary`

### React Query hook

- `app/src/lib/queries.ts`
  - `useCostSummary(timeframe)`

---

## Event schema (CostEvent)

The key fields you’ll care about when extending the system:

- `created_at`: timestamp
- `request_id`: links multiple cost events (STT + LLM) to a single transcription attempt
- `kind`: `stt` or `llm`
- `provider`: string (e.g. `"openai"`)
- `model`: optional string
- `status`: `success | error | cancelled`

Usage payloads:

- `audio_duration_secs`: optional
- `tokens`: optional
  - `input_tokens`
  - `output_tokens`
  - `cached_input_tokens`
  - `input_audio_tokens`
  - `output_audio_tokens`

Cost fields:

- `estimated_cost_usd_micros`: optional (the number UI totals use today)
- `estimated_cost_breakdown_openai`: optional (OpenAI-specific breakdown)

**Important behavior:**

- Events without `estimated_cost_usd_micros` still get recorded, but the current total spend excludes them.

---

## Adding a new provider/model (step-by-step)

There are two broad paths:

1. **Token-based cost estimation** (best for LLMs, and any provider that reports usage)
2. **Time-based cost estimation** (best for STT that charges per minute)

### 1) Add pricing + estimator for the provider

Create a pricing module similar to OpenAI’s.

Suggested location:

- `app/src-tauri/src/cost/<provider>.rs`
- and export it from `app/src-tauri/src/cost/mod.rs`

Implement:

- a function to resolve model → price rates (e.g. per-1M tokens)
- an estimator that turns usage → USD micros

**Recommendation:** keep all costs in **USD micros** (`u64`) so:

- adding sums is safe and exact
- no floating rounding drift

### 2) Extract usage from provider responses

For each provider, we need a parser for the provider’s response JSON:

- Add `parse_<provider>_usage_from_response_json(v: &serde_json::Value) -> Option<UsageStruct>`

OpenAI already has:

- `parse_openai_usage_from_response_json` in `stats.rs`

Where to get response JSON today:

- from `RequestLog`:
  - `stt_response_json`
  - `llm_response_json`

> Note: request logs are in-memory, but we only use them as a convenient place to capture response payloads at the moment of writing the persisted stats event.

### 3) Wire provider into `write_cost_events_for_current_request`

In:

- `app/src-tauri/src/commands/recording.rs`

Extend the logic:

- check `stt_provider` / `llm_provider`
- when it matches your provider:
  - parse usage from response JSON
  - call your estimator
  - set `estimated_cost_usd_micros`

For STT per-minute pricing:

- ensure `audio_duration_secs` is set
  - we compute this from WAV bytes (`wav_duration_secs`)

### 4) Confirm events are being written

Events append during transcription flows:

- stop + transcribe
- retry transcription

You can verify by locating:

- `<app_data_dir>/stats/cost-events-*.jsonl`

---

## Adding new UI features (recommended pattern)

The Cost tab is expected to grow.

Guidelines:

1. Keep the **top “Total spend”** card stable.
2. Add additional cards/sections below it (charts, breakdown tables, per-provider totals, budgets, etc.).
3. Prefer fetching aggregated views via new Tauri commands over parsing JSONL directly in TS.

Suggested next expansions:

- Spend breakdown by:
  - provider
  - model
  - STT vs LLM
- Trend chart (daily totals)
- “Unknown/unpriced usage” warning (events missing `estimated_cost_usd_micros`)
- Export CSV/JSON

---

## Adding a new aggregation endpoint (backend → frontend)

### Backend

Add a new command in:

- `app/src-tauri/src/commands/stats.rs`

Register it in:

- `app/src-tauri/src/commands/mod.rs`
- `app/src-tauri/src/lib.rs` invoke handler list

Keep the command payloads small and UI-friendly.

### Frontend

Add to:

- `app/src/lib/tauri.ts` (typed wrapper)
- `app/src/lib/queries.ts` (React Query hook)

Then render in:

- `app/src/components/usageStats/CostTab.tsx`

---

## Timeframes (current)

Supported values (string):

- `"24h"`, `"7d"`, `"30d"`, `"90d"`, `"all"`

Backend default fallback if unknown:

- `"30d"`

---

## Known limitations (intentionally accepted for v1)

- Not all providers/models will have usage fields available; events may be missing `estimated_cost_usd_micros`.
- JSONL scanning is fine at current expected scale, but could be migrated to SQLite if needed.
- Cost accuracy depends on:
  - correct model name mapping
  - correct pricing tables
  - presence of provider usage reporting

---

## Quick checklist for contributors

When adding a provider/model cost:

- [ ] Add pricing table + estimator in `src-tauri/src/cost/`
- [ ] Add usage parsing from response JSON
- [ ] Hook provider into `write_cost_events_for_current_request`
- [ ] Ensure cost gets written into `estimated_cost_usd_micros`
- [ ] Add aggregation endpoint(s) as needed for UI
- [ ] Extend Cost tab UI with a new card/section (keep Total spend at top)
