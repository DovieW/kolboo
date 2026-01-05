# Program Profiles, Presets, and Intent Routing

This document is a research + design proposal for evolving Kolboo’s current per-program settings system into:

1. **Multiple presets per program profile** (quick switching)
2. An **optional router layer** that can automatically pick a preset based on what the user dictates (their “question” / “intent”)
3. **Embedding-based** and **LLM-based** routing strategies, configurable in Settings

**Status**: Research & Design (no code changes in this phase)

**Date**: 2026-01-03

**Primary user goal** (paraphrased): “When I’m in the same program, I want different behaviors (prompts/models/output) depending on what I’m saying, without manually switching settings every time.”

---

## Current state (what the code actually does today)

### Storage and types

**The per-program system is persisted in** `settings.json` (Tauri store plugin) under:

- Key: `rewrite_program_prompt_profiles`
- Frontend type: `RewriteProgramPromptProfile` in `app/src/lib/tauri.ts`
- Backend type: `RewriteProgramPromptProfile` in `app/src-tauri/src/settings.rs`

**Important mismatch to note:**

- The frontend `RewriteProgramPromptProfile` includes UI/output override fields (e.g. `overlay_mode`, `output_mode`, `sound_enabled`).
- The Rust backend struct currently _does not_ include those UI fields. The backend currently applies only:
  - prompt section overrides
  - rewrite enable toggle
  - STT/LLM provider/model overrides
  - provider-specific thinking knobs

So: as of today, “profiles override UI settings” is _not_ true end-to-end.

### Where profile selection happens

**Primary pipeline selection is path-based.** The active profile is chosen by comparing the foreground executable path with `program_paths`:

- `app/src-tauri/src/pipeline.rs`: `select_profile_for_foreground_app()`
  - normalizes Windows paths (case-insensitive and `/` → `\`)
  - picks the first profile whose `program_paths` contains the foreground path

**Notably:** the pipeline selects the profile **at transcription time**, not necessarily at recording start. This is intentional because the overlay window can steal focus.

### How profile overrides apply

The backend transforms persisted profiles into runtime profiles:

- persisted type: `settings::RewriteProgramPromptProfile`
- runtime type: `llm::ProgramPromptProfile` (in `app/src-tauri/src/llm/mod.rs`)

Prompt resolution is currently:

1. Start from built-in default prompt sections
2. Apply global overrides (`cleanup_prompt_sections`)
3. Apply per-program override sections (`RewriteProgramPromptProfile.cleanup_prompt_sections`)

Provider/model resolution is currently:

- Per-program overrides fall back to global defaults.
- If a per-program provider is selected but missing an API key, the pipeline attempts a safe fallback to the global provider.

### Existing “manual override” hooks

There is a backend-only notion of forcing a profile in retry/test flows:

- `profile_id_override` in retry transcription code paths (see `pipeline.rs`)

There is **no end-user runtime “use preset X for this transcription”** concept yet (that will be part of the proposed system below).

### Embeddings: current state

There is **no existing embeddings subsystem** in the workspace today (no embedding provider, no embedding models list, no vector store).

---

## Why the existing “preset” idea is not enough

Presets alone solve: “I want multiple named setups per program and I can manually choose one.”

But the new requirement is: **automatic selection** based on the dictation content.

That requires a router layer that:

- observes the user’s text (usually post-STT)
- decides which preset is best
- applies that preset for the current transcription (and/or for the next one)

---

## Terminology (replace “config” with clearer words)

The word “config” is too generic. We need two distinct concepts:

1. **Program matching scope**: “when I’m in VS Code, apply these rules”
2. **Within that scope**: “pick a behavior based on what I’m doing/saying”

### Recommended names

| Concept                                       | Recommended name (UI)  | Recommended name (code/docs)                       | Notes                                                                 |
| --------------------------------------------- | ---------------------- | -------------------------------------------------- | --------------------------------------------------------------------- |
| Foreground app matching                       | **Program Profile**    | `ProgramProfile` (or keep existing name initially) | Matches executable paths. This is what you currently call “profile”.  |
| A named behavior set inside a program profile | **Preset** or **Mode** | `Preset` (or `ModePreset`)                         | “Preset” is familiar; “Mode” reads well (“Budget mode”, “Code mode”). |
| Automatic choice mechanism                    | **Intent Router**      | `IntentRouter`                                     | Reads as “route by intent”.                                           |

### Optional (future) cleanup: split matching vs settings

Conceptually, the cleanest model is:

- **App Rule**: matches executable paths
- **Profile**: reusable settings bundle

But today’s persisted shape merges these. This doc sticks to the current shape (“Program Profile contains program paths”) and proposes a migration path later.

---

## Proposed architecture: Presets + Intent Router

### High-level resolution order

At transcription time, we want a deterministic precedence order:

1. **User “locked preset” override** (temporary, for “while I’m transcribing”)
2. **Router-selected preset** (automatic)
3. **Default preset** for that program profile
4. **Program profile base settings** (if we keep “base overrides” on the profile itself)
5. **Global settings** (Default scope)

### Why “default preset” is necessary

Users need:

- A stable baseline for each program (“this is my normal VS Code behavior”)
- A one-click way to **turn routing off** (“always use default”)

So each program profile must designate one preset as the default.

---

## Data model (conceptual)

### Program Profile (existing concept, extended)

The persisted entity today is `RewriteProgramPromptProfile`.

We extend it conceptually with:

- `presets`: list of presets/modes
- `default_preset_id`: which preset is the “normal” default
- `router`: optional router configuration

### Preset (a named set of overrides)

A preset contains the same override fields as the program profile (prompt overrides, provider/model overrides, etc), plus optional routing hints.

Routing hints are critical for embedding-based routing.

Example hints:

- “write a professional email”
- “summarize meeting notes”
- “dictate code”

### Router configuration

Router must be easy to turn off and must be configurable.

Minimal set of fields:

- `enabled: boolean`
- `strategy: "off" | "embeddings" | "llm"`
- `fallback: "default_preset" | "ask_user" | "no_routing"` (MVP should use default_preset)
- `locked_preset_id?: string` (runtime override; may be stored separately from persistent settings)

---

## Routing strategies

### Strategy A: Embedding-based routing (recommended default)

**How it works (MVP):**

1. Each preset has a list of `routing_hints` (short example utterances).
2. Compute embeddings for those hints and cache them.
3. When a transcript arrives, compute an embedding for the transcript.
4. Compute cosine similarity against each hint embedding.
5. Choose the preset with the highest similarity if it exceeds a threshold; otherwise use default.

**Similarity metric:** cosine similarity.

**Config knobs (reasonable MVP):**

- embedding provider: `openai` (initially)
- embedding model: `text-embedding-3-small` or `text-embedding-3-large`
- threshold: e.g. 0.75 (tunable)
- margin: require top score − runner-up ≥ 0.05 to avoid close calls
- max text length: truncate transcript to avoid huge embedding requests

**OpenAI embeddings models (confirmed):**

- `text-embedding-3-small`
- `text-embedding-3-large`

OpenAI endpoint: `POST /v1/embeddings`.

**Notes:**

- This is fast, cheap, and deterministic.
- It’s also “explainable”: we can show scores in UI if needed.
- It avoids an extra LLM call for every transcription.

### Strategy B: LLM-based routing (more flexible)

**How it works (MVP):**

1. Create a short system prompt: “You are a classifier. Choose one preset id.”
2. Provide: transcript + available preset names/descriptions + optional examples.
3. LLM returns a preset id in a strict JSON shape.

**Config knobs:**

- router LLM provider/model (can differ from rewrite LLM)
- max tokens small
- temperature = 0
- fallback behavior if LLM fails

**Notes:**

- Better when intent is nuanced.
- But adds latency and cost, and error modes are weirder.

---

## Practical pipeline constraints (important)

### Routing can’t influence STT for the _current_ recording (today)

Routing input is the transcript, which is only available **after** STT. Today, STT provider/model is selected before transcription begins.

Implication:

- In MVP, routing should primarily control:
  - LLM rewrite prompts (most important)
  - LLM provider/model/thinking knobs
  - output behavior (once backend supports those overrides)

We can still allow presets to include STT overrides, but they would naturally apply to:

- the **next recording**, or
- a future “two-pass” system (STT -> route -> optional re-STT), which is probably too expensive.

---

## UX requirements

### Requirements from the request

- Users can set up multiple presets for a given program profile.
- One preset is the default for that profile.
- Router can be turned off easily.
  - When off: always use the default preset.
- User can temporarily pick/lock a preset “while transcribing.”
- Router strategy is configurable: embeddings-based or LLM-based.
- Must add embedding models (start with OpenAI small/large).

### Proposed UX surfaces (MVP)

1. **Settings → Program Profiles**

   - manage program paths
   - manage presets (create/edit/delete)
   - set default preset
   - configure router

2. **Quick override while dictating**
   - tray menu: “Active Program Profile → Presets → (Lock for next transcription / Lock until unset)”
   - future hotkey overlay: quick switcher

---

## Files and touchpoints (for future implementation)

This section lists where changes would likely go (no code changes in this phase).

### Backend (Rust)

- `app/src-tauri/src/settings.rs`

  - extend persisted profile structures with presets and router settings
  - add serde defaults for backward compatibility

- `app/src-tauri/src/llm/mod.rs`

  - consider whether runtime `ProgramPromptProfile` should become a richer “effective config” carrier

- `app/src-tauri/src/pipeline.rs`

  - insert “resolve effective preset” step after STT (for LLM stage)
  - support a runtime override (“locked preset”) for current transcription

- `app/src-tauri/src/commands/config.rs`

  - add embedding provider/model availability endpoints (optional)
  - include router settings in pipeline config sync

- New modules (proposed)
  - `app/src-tauri/src/embeddings/*` (OpenAI embeddings client, similarity utilities)
  - `app/src-tauri/src/router/*` (routing strategies and decision logic)

### Frontend (TypeScript/React)

- `app/src/lib/tauri.ts`

  - extend types with presets/router config
  - add store getters/setters (still via plugin-store)

- `app/src/lib/modelOptions.ts`

  - add `EMBEDDING_MODELS` list (OpenAI: small/large)

- `app/src/components/settings/ProgramsModal.tsx`

  - current modal handles program paths; extend to include preset + router management, or create a new `PresetsModal.tsx`.

- `app/src/components/settings/PromptSettings.tsx`
  - show preset selector (manual selection)
  - show router on/off and strategy selection

---

## Migration and compatibility

Key design constraint: existing installs must keep working.

Backward compatibility approach:

- If `presets` is missing: treat as empty.
- If `default_preset_id` is missing: treat “use base program profile settings” as default behavior.
- If router config is missing: router disabled.

---

## Open questions / decisions

1. **Preset naming in UI**: “Preset” vs “Mode”.

   - Recommendation: **Preset** (familiar), allow users to name them “Budget mode”, “Code mode”, etc.

2. **Where to store the temporary lock override**:

   - In-memory only (resets on app restart)
   - Persisted in `settings.json` (survives restart)
   - Recommendation: MVP in-memory; optionally persist “last locked preset” per program profile.

3. **Explainability**:

   - For embeddings routing, should we show scores / “why it picked this preset”?
   - Recommendation: log-only at first; add UI debug later.

4. **Privacy and logging**:
   - Embedding/LLM routing sends transcript text to providers.
   - Recommendation: reuse existing provider opt-in assumptions and document it clearly.

---

## Next steps

1. Confirm terminology choice (Preset vs Mode, Program Profile naming).
2. Decide MVP routing scope (LLM stage only vs also output/UI behavior).
3. Design the concrete persisted schema for presets + router in `settings.json`.
4. Implement router as an opt-in feature behind a clear toggle.
