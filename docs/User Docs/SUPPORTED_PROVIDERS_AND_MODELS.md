# Supported Providers & Models

This doc lists the **provider IDs** and **model IDs** currently supported by the app’s settings pickers.

Source of truth:

- Provider list: `app/src-tauri/src/commands/config.rs` (`STT_PROVIDERS`, `LLM_PROVIDERS`)
- Model lists (shown in the UI): `app/src/lib/modelOptions.ts` (`STT_MODELS`, `LLM_MODELS`)

Notes:

- Some providers support **dynamic model discovery** (e.g. Ollama, Local Whisper). Those do not have a fixed list here.
- “Supported” here means “wired up end-to-end and selectable in Settings” (not “every possible upstream model string”).

---

## Speech-to-Text (STT)

### `openai` (OpenAI)

- `gpt-4o-transcribe` — GPT-4o Transcribe
- `gpt-4o-mini-transcribe` — GPT-4o Mini Transcribe
- `whisper-1` — Whisper-1

### `aquavoice` (Aquavoice)

- `avalon-v1-en` — Avalon v1

### `groq` (Groq)

- `whisper-large-v3-turbo` — Whisper Large V3 Turbo
- `whisper-large-v3` — Whisper Large V3

### `assemblyai` (AssemblyAI)

- `universal` — Universal
- `slam-1` — Slam-1
- `best` — Best (Legacy)

### `speechmatics` (Speechmatics)

- `enhanced` — Enhanced
- `standard` — Standard

### `deepgram` (Deepgram)

- `nova-3` — Nova 3
- `nova-2` — Nova 2
- `nova` — Nova
- `enhanced` — Enhanced
- `base` — Base

### `whisper` (Local Whisper)

- No fixed list here (models are managed locally; the Settings picker does not enumerate Whisper models from `modelOptions.ts`).

---

## Language Models (LLM)

### `cerebras` (Cerebras)

- `llama-3.3-70b` — Llama 3.3 70B
- `llama3.1-8b` — Llama 3.1 8B
- `gpt-oss-120b` — GPT-OSS 120B
- `qwen-3-32b` — Qwen 3 32B
- `qwen-3-235b-a22b-instruct-2507` — Qwen 3 235B Instruct (Preview)
- `zai-glm-4.7` — GLM 4.7 (Preview)
- `zai-glm-4.6` — GLM 4.6 (Preview)

### `openai` (OpenAI, LLM)

- `gpt-5.2` — GPT-5.2
- `gpt-5.1` — GPT-5.1
- `gpt-5` — GPT-5
- `gpt-5-mini` — GPT-5 Mini
- `gpt-5-nano` — GPT-5 Nano
- `gpt-4.1` — GPT-4.1
- `gpt-4.1-mini` — GPT-4.1 Mini
- `gpt-4.1-nano` — GPT-4.1 Nano

### `gemini` (Google AI Studio)

- `models/gemini-3-pro-preview` — Gemini 3 Pro (Preview)
- `models/gemini-3-flash-preview` — Gemini 3 Flash (Preview)
- `gemini-2.5-pro` — Gemini 2.5 Pro
- `gemini-2.5-flash` — Gemini 2.5 Flash
- `gemini-2.5-flash-lite` — Gemini 2.5 Flash-Lite

### `anthropic` (Anthropic)

- `claude-sonnet-4-5` — Claude Sonnet 4.5
- `claude-haiku-4-5` — Claude Haiku 4.5
- `claude-opus-4-5` — Claude Opus 4.5
- `claude-3-5-haiku-latest` — Claude 3.5 Haiku
- `claude-3-5-sonnet-latest` — Claude 3.5 Sonnet
- `claude-3-opus-latest` — Claude 3 Opus

### `groq` (Groq, LLM)

- `llama-3.3-70b-versatile` — Llama 3.3 70B Versatile
- `llama-3.1-8b-instant` — Llama 3.1 8B Instant
- `openai/gpt-oss-120b` — GPT-OSS 120B
- `openai/gpt-oss-20b` — GPT-OSS 20B

### `ollama` (Ollama)

- No fixed list here (models are discovered from what you have installed in Ollama).
